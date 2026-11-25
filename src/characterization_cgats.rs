//! Single-table CGATS/Argyll CTI3 characterization adapter.
use crate::{R,characterization::{Chart,Patch}};
use std::collections::{BTreeMap,HashSet};
fn tokens(line:&str)->R<Vec<String>>{
 let mut out=Vec::new();let mut token=String::new();let mut quoted=false;let mut active=false;
 for c in line.chars(){
  if c=='"'{quoted=!quoted;active=true;}
  else if !quoted&&c=='#'{break}
  else if !quoted&&c.is_whitespace(){if active{out.push(std::mem::take(&mut token));active=false;}}
  else{token.push(c);active=true;}
 }
 if quoted{return Err("Unterminated CGATS quote".into())}
 if active{out.push(token);}Ok(out)
}
fn num(s:&str)->R<f64>{let v=s.parse::<f64>().map_err(|_|"Invalid CGATS number")?;if !v.is_finite(){return Err("Nonfinite CGATS number".into())}Ok(v)}
pub(crate) fn read(text:&str,n:usize,cmyk:bool)->R<Chart>{
 if text.len()>32*1024*1024{return Err("CGATS exceeds 32 MiB".into())}
 let mut state=0;let mut fields=Vec::new();let mut rows=Vec::new();let mut meta=BTreeMap::new();
 for line in text.lines(){
  let v=tokens(line)?;if v.is_empty(){continue}
  match v[0].as_str(){
   "BEGIN_DATA_FORMAT" if state==0=>state=1,
   "END_DATA_FORMAT" if state==1=>state=2,
   "BEGIN_DATA" if state==2=>state=3,
   "END_DATA" if state==3=>state=4,
   "BEGIN_DATA_FORMAT"|"END_DATA_FORMAT"|"BEGIN_DATA"|"END_DATA"=>return Err("Multiple or malformed CGATS tables".into()),
   _=>match state{
    0|2=>{if v.len()==2{meta.insert(v[0].clone(),v[1].clone());}},
    1=>fields.extend(v),
    3=>{if rows.len()>=100000{return Err("Too many CGATS patches".into())}rows.push(v);},
    _=>return Err("Unexpected data after CGATS table".into())
   }
  }
 }
 if state!=4||fields.is_empty()||rows.is_empty(){return Err("Missing CGATS data table".into())}
 let mut seen=HashSet::new();if fields.iter().any(|f|!seen.insert(f)){return Err("Duplicate CGATS field".into())}
 for (key,actual) in [("NUMBER_OF_FIELDS",fields.len()),("NUMBER_OF_SETS",rows.len())]{
  if let Some(s)=meta.get(key){if s.parse::<usize>().ok()!=Some(actual){return Err(format!("Invalid {key}"))}}
 }
 let index=|s:&str|fields.iter().position(|f|f==s).ok_or_else(||format!("Missing CGATS field {s}"));
 let id=index("SAMPLE_ID")?;
 let device=if cmyk{["CMYK_C","CMYK_M","CMYK_Y","CMYK_K"].iter().map(|s|index(s)).collect::<R<Vec<_>>>()?}else{
  // Only an explicit CH1..CHn physical mapping is supported. Do not
  // infer ink identity or reorder arbitrary vendor fields.
  (1..=n).map(|j|index(&format!("CH{j}"))).collect::<R<Vec<_>>>()?
 };
 if device.len()!=n{return Err("CGATS device channel count mismatch".into())}
 let lab=["LAB_L","LAB_A","LAB_B"].iter().map(|s|index(s)).collect::<R<Vec<_>>>().ok();
 let xyz=["XYZ_X","XYZ_Y","XYZ_Z"].iter().map(|s|index(s)).collect::<R<Vec<_>>>().ok();
 let is_d50=meta.get("ILLUMINANT").is_some_and(|s|s.eq_ignore_ascii_case("D50"))&&meta.get("OBSERVER").is_some_and(|s|matches!(s.as_str(),"2"|"2_DEGREE"|"1931"));
 let mode=meta.get("MEASUREMENT_CONDITION").cloned().unwrap_or_else(||"Unspecified".into());
 let mut spectral=fields.iter().enumerate().filter_map(|(i,s)|s.strip_prefix("SPEC_").and_then(|v|v.parse::<u32>().ok()).map(|w|(w,i))).collect::<Vec<_>>();
 spectral.sort_unstable();
 let step=if spectral.len()>=2{let s=spectral[1].0-spectral[0].0;if s>0&&spectral.windows(2).all(|r|r[1].0-r[0].0==s){Some(s)}else{None}}else{None};
 let norm=meta.get("SPECTRAL_NORM").map(|v|num(v)).transpose()?;
 // Spectral scale must be declared; retain no ambiguous spectrum.
 if !spectral.is_empty()&&(step.is_none()||norm.is_none_or(|v|v<=0.)){return Err("CGATS spectra require a uniform wavelength grid and positive SPECTRAL_NORM".into())}
 let mut patches=Vec::with_capacity(rows.len());let mut ids=HashSet::new();
 for (i,row) in rows.iter().enumerate(){
  if row.len()!=fields.len(){return Err("CGATS record width mismatch".into())}
  if !ids.insert(row[id].clone()){return Err("Duplicate CGATS SAMPLE_ID".into())}
  let inks=device.iter().map(|&j|num(&row[j])).collect::<R<Vec<_>>>()?;
  if inks.iter().any(|&v|!(0.0..=100.).contains(&v)){return Err("CGATS ink percentages outside 0–100".into())}
  let measured_lab=if let Some(j)=&lab{Some([num(&row[j[0]])?,num(&row[j[1]])?,num(&row[j[2]])?])}else if is_d50{
   if let Some(j)=&xyz{
    use lcms2::{CIEXYZ,CIEXYZExt};
    let v=CIEXYZ{X:num(&row[j[0]])?/100.,Y:num(&row[j[1]])?/100.,Z:num(&row[j[2]])?/100.};
    if [v.X,v.Y,v.Z].iter().any(|&x|x<0.){return Err("Negative measured XYZ".into())}
    let l=v.to_lab(&CIEXYZ{X:0.9642,Y:1.,Z:0.8249});Some([l.L,l.a,l.b])
   }else{None}
  }else{None};
  let spectrum=if let Some(scale)=norm{if spectral.is_empty(){None}else{let v=spectral.iter().map(|&(_,j)|num(&row[j]).map(|v|v/scale)).collect::<R<Vec<_>>>()?;if v.iter().any(|&x|x<0.||!x.is_finite()){return Err("Invalid CGATS reflectance".into())}Some(v)}}else{None};
  patches.push(Patch{id:row[id].clone(),page:1+i/400,row:i%400/20,column:i%20,inks,spectrum,start_nm:spectral.first().map(|v|v.0),step_nm:step,measurement:mode.clone(),predicted_lab:[0.;3],measured_lab,absolute_lab:None,comparison_lab:None,delta_e:None,measurement_basis:if is_d50{"Declared D50/2°".into()}else{"Colorimetric conditions unspecified".into()}});
 }
 Ok(Chart{source:"CGATS characterization table".into(),original_layout:false,patches})
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn measured_and_quoted_ids(){
  let c=read("CTI3\nILLUMINANT D50\nOBSERVER 2\nNUMBER_OF_FIELDS 8\nNUMBER_OF_SETS 1\nBEGIN_DATA_FORMAT\nSAMPLE_ID CMYK_C CMYK_M CMYK_Y CMYK_K LAB_L LAB_A LAB_B\nEND_DATA_FORMAT\nBEGIN_DATA\n\"Patch 1\" 0 50 25 0 42 -3 12\nEND_DATA\n",4,true).unwrap();
  assert_eq!(c.patches[0].id,"Patch 1");assert_eq!(c.patches[0].measured_lab,Some([42.,-3.,12.]));assert!(!c.original_layout);
 }
 #[test]fn refuse_duplicate_fields(){assert!(read("CGATS.17\nBEGIN_DATA_FORMAT\nSAMPLE_ID SAMPLE_ID\nEND_DATA_FORMAT\nBEGIN_DATA\na b\nEND_DATA",4,true).is_err());}
}
