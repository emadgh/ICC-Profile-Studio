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
pub(crate) fn read(text:&str,n:usize,cmyk:bool)->R<Chart>{read_with_names(text,n,cmyk,None)}
pub(crate) fn read_with_names(text:&str,n:usize,cmyk:bool,names:Option<&[String]>)->R<Chart>{
 if !(2..=15).contains(&n)||cmyk&&n!=4{return Err("Invalid CGATS channel count".into())}
 if text.len()>32*1024*1024{return Err("CGATS exceeds 32 MiB".into())}
 let mut state=0;let mut fields=Vec::new();let mut rows=Vec::new();let mut meta=BTreeMap::new();let mut process=BTreeMap::<usize,String>::new();let mut iso28178=false;
 for line in text.lines(){
  let v=tokens(line)?;if v.is_empty(){continue}
  match v[0].as_str(){
   "ISO28178" if state==0=>iso28178=true,
   "PROCESSCOLOR_ID" if state==0||state==2=>{if v.len()!=2{return Err("Invalid PROCESSCOLOR_ID".into())}let mut parts=v[1].split_whitespace();let count=parts.next().and_then(|x|x.parse::<usize>().ok()).ok_or("Invalid PROCESSCOLOR_ID count")?;let j=parts.next().and_then(|x|x.parse::<usize>().ok()).ok_or("Invalid PROCESSCOLOR_ID index")?;let name=parts.collect::<Vec<_>>().join(" ");if count!=n||j==0||j>n||name.is_empty()||process.insert(j,name).is_some(){return Err("Invalid or duplicate PROCESSCOLOR_ID".into())}},
   "BEGIN_DATA_FORMAT" if state==0=>state=1,
   "END_DATA_FORMAT" if state==1=>state=2,
   "BEGIN_DATA" if state==2=>state=3,
   "END_DATA" if state==3=>state=4,
   "BEGIN_DATA_FORMAT"|"END_DATA_FORMAT"|"BEGIN_DATA"|"END_DATA"=>return Err("Multiple or malformed CGATS tables".into()),
   _=>match state{
    0|2=>{if v.len()==2{if matches!(v[0].as_str(),"ILLUMINANT"|"ILLUMINATION_NAME"|"OBSERVER"|"OBSERVER_ANGLE"|"MEASUREMENT_CONDITION"|"FILTER"|"SPECTRAL_NORM"|"NUMBER_OF_FIELDS"|"NUMBER_OF_SETS")&&meta.get(&v[0]).is_some_and(|prior|prior!=&v[1]){return Err(format!("Conflicting CGATS metadata {}",v[0]))}meta.insert(v[0].clone(),v[1].clone());}},
    1=>{if fields.len()+v.len()>1100{return Err("Too many CGATS fields".into())}fields.extend(v);},
    3=>{if v.len()!=fields.len(){return Err("CGATS record width mismatch".into())}if rows.len()>=100000{return Err("Too many CGATS patches".into())}rows.push(v);},
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
 // ISO 28178 uses PC<n>_<physical index>. Accept only an explicit
 // count and complete ordered ink identity matching the loaded ICC.
 let pc=iso28178&&fields.iter().any(|f|f.starts_with("PC")&&f.contains('_'));
 let id=fields.iter().position(|f|f=="SAMPLE_ID");
 if id.is_none()&&!pc{return Err("Missing CGATS field SAMPLE_ID".into())}
 let device=if pc{
  if fields.iter().any(|f|f.starts_with("CH")||f.starts_with("CMYK_")){return Err("Ambiguous process-color field mapping".into())}
  let names=names.filter(|v|v.len()==n).ok_or("ISO28178 requires explicit profile channel names")?;
  for j in 1..=n{
   let vendor=meta.get(&format!("LGOMCCHANNEL{j:02}")).and_then(|s|s.split_once("InkName = '")).and_then(|(_,s)|s.split_once('\'')).map(|(name,_)|name);
   let declared=process.get(&j).map(String::as_str);
   if declared.is_some()&&vendor.is_some()&&!declared.unwrap().eq_ignore_ascii_case(vendor.unwrap()){return Err(format!("Conflicting process color name at channel {j}"))}
   let name=declared.or(vendor).ok_or_else(||format!("Missing explicit process color name at channel {j}"))?;
   if !name.eq_ignore_ascii_case(&names[j-1]){return Err(format!("Process color mapping differs from ICC at channel {j}"))}
  }
  let indices=(1..=n).map(|j|index(&format!("PC{n}_{j}"))).collect::<R<Vec<_>>>()?;
  if fields.iter().filter(|f|f.starts_with("PC")&&f.contains('_')).count()!=n{return Err("Unexpected ISO28178 process-color fields".into())}indices
 }else if cmyk{["CMYK_C","CMYK_M","CMYK_Y","CMYK_K"].iter().map(|s|index(s)).collect::<R<Vec<_>>>()?}else{
  (1..=n).map(|j|index(&format!("CH{j}"))).collect::<R<Vec<_>>>()?
 };
 if device.len()!=n{return Err("CGATS device channel count mismatch".into())}
 let lab=["LAB_L","LAB_A","LAB_B"].iter().map(|s|index(s)).collect::<R<Vec<_>>>().ok();
 if lab.is_none()&&["LAB_L","LAB_A","LAB_B"].iter().any(|s|fields.iter().any(|f|f.as_str()==*s)){return Err("Incomplete CGATS Lab fields".into())}
 let xyz=["XYZ_X","XYZ_Y","XYZ_Z"].iter().map(|s|index(s)).collect::<R<Vec<_>>>().ok();
 if xyz.is_none()&&["XYZ_X","XYZ_Y","XYZ_Z"].iter().any(|s|fields.iter().any(|f|f==s)){return Err("Incomplete CGATS XYZ fields".into())}
 for (a,b) in [("ILLUMINANT","ILLUMINATION_NAME"),("OBSERVER","OBSERVER_ANGLE"),("MEASUREMENT_CONDITION","FILTER")]{if let (Some(a),Some(b))=(meta.get(a),meta.get(b)){if !a.eq_ignore_ascii_case(b){return Err("Conflicting measurement metadata aliases".into())}}}
 let is_d50=meta.get("ILLUMINANT").or_else(||if iso28178{meta.get("ILLUMINATION_NAME")}else{None}).is_some_and(|s|s.eq_ignore_ascii_case("D50"))&&meta.get("OBSERVER").or_else(||if iso28178{meta.get("OBSERVER_ANGLE")}else{None}).is_some_and(|s|matches!(s.as_str(),"2"|"2_DEGREE"|"1931"));
 let mode=meta.get("MEASUREMENT_CONDITION").or_else(||if iso28178{meta.get("FILTER").filter(|s|matches!(s.as_str(),"M0"|"M1"|"M2"|"M3"))}else{None}).cloned().unwrap_or_else(||"Unspecified".into());
 let mut spectral=fields.iter().enumerate().filter_map(|(i,s)|s.strip_prefix("SPEC_").and_then(|v|v.parse::<u32>().ok()).map(|w|(w,i))).collect::<Vec<_>>();
 if spectral.len()>1024||fields.iter().filter(|f|f.starts_with("SPEC_")).count()!=spectral.len(){return Err("Invalid spectral field names or count".into())}
 spectral.sort_unstable();
 let step=if spectral.len()>=2{let s=spectral[1].0-spectral[0].0;if s>0&&spectral.windows(2).all(|r|r[1].0-r[0].0==s){Some(s)}else{None}}else{None};
 let norm=meta.get("SPECTRAL_NORM").map(|v|num(v)).transpose()?;
 // Spectral scale must be declared; retain no ambiguous spectrum.
 if !spectral.is_empty()&&(step.is_none()||norm.is_none_or(|v|v<=0.)){return Err("CGATS spectra require a uniform wavelength grid and positive SPECTRAL_NORM".into())}
 let mut patches=Vec::with_capacity(rows.len());let mut ids=HashSet::new();
 for (i,row) in rows.iter().enumerate(){
  if row.len()!=fields.len(){return Err("CGATS record width mismatch".into())}
  let patch_id=id.map(|j|row[j].clone()).unwrap_or_else(||format!("row-{:06}",i+1));if patch_id.trim().is_empty()||!ids.insert(patch_id.clone()){return Err("Duplicate CGATS SAMPLE_ID".into())}
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
  patches.push(Patch{id:patch_id,page:1+i/400,row:i%400/20,column:i%20,inks,spectrum,start_nm:spectral.first().map(|v|v.0),step_nm:step,measurement:mode.clone(),predicted_lab:[0.;3],measured_lab,absolute_lab:None,comparison_lab:None,delta_e:None,measurement_basis:if is_d50{"Declared D50/2°".into()}else{"Colorimetric conditions unspecified".into()}});
 }
 Ok(Chart{source:"CGATS characterization table".into(),original_layout:false,patches})
}
/// Export a single homogeneous table in the subset accepted by read().
/// Never promote model predictions or spectral estimates into measured Lab.
pub(crate) fn write(c:&Chart,n:usize,cmyk:bool)->R<String>{
 use std::fmt::Write;
 if c.patches.is_empty()||c.patches.len()>100000||!(2..=15).contains(&n)||(cmyk&&n!=4){return Err("Invalid characterization export dimensions".into())}
 let first=&c.patches[0];
 let lab=first.measured_lab.is_some();
 let spectra=first.spectrum.is_some();
 let mode=&first.measurement;
 let quote=|s:&str|->R<String>{
  if s.is_empty()||s.chars().any(|x|x=='"'||x.is_control()){return Err("CGATS cannot safely encode an empty ID, quote or control character; use CSV".into())}
  Ok(format!("\"{s}\""))
 };
 let grid=if spectra{
  let start=first.start_nm.ok_or("Missing spectral start")?;
  let step=first.step_nm.filter(|&s|s>0).ok_or("Missing spectral increment")?;
  let count=first.spectrum.as_ref().unwrap().len();
  if !(2..=1024).contains(&count)||start.checked_add(step.checked_mul((count-1) as u32).ok_or("Spectral grid overflow")?).is_none_or(|end|end>2500){return Err("Invalid spectral grid".into())}
  Some((start,step,count))
 }else{None};
 let mut fields=vec!["SAMPLE_ID".to_owned()];
 if cmyk{fields.extend(["CMYK_C","CMYK_M","CMYK_Y","CMYK_K"].map(str::to_owned));}else{fields.extend((1..=n).map(|j|format!("CH{j}")));}
 if lab{fields.extend(["LAB_L","LAB_A","LAB_B"].map(str::to_owned));}
 if let Some((start,step,count))=grid{fields.extend((0..count).map(|j|format!("SPEC_{}",start+step*j as u32)));}
 let mut out=String::from("CGATS.17\n");
 if !lab&&!spectra{out.push_str("DESCRIPTOR \"Device recipes only; no measurements\"\n");}
 if lab{out.push_str("ILLUMINANT D50\nOBSERVER 2\n");}
 if !mode.is_empty()&&mode!="Unspecified"{writeln!(out,"MEASUREMENT_CONDITION {}",quote(mode)?).unwrap();}
 if spectra{out.push_str("SPECTRAL_NORM 1\n");}
 writeln!(out,"NUMBER_OF_FIELDS {}\nBEGIN_DATA_FORMAT\n{}\nEND_DATA_FORMAT\nNUMBER_OF_SETS {}\nBEGIN_DATA",fields.len(),fields.join(" "),c.patches.len()).unwrap();
 let mut ids=HashSet::new();
 for p in &c.patches{
  if !ids.insert(p.id.as_str()){return Err("Duplicate SAMPLE_ID; use CSV to preserve original positions".into())}
  if p.inks.len()!=n||p.inks.iter().any(|v|!v.is_finite()||!(0.0..=100.).contains(v)){return Err("Invalid device percentages".into())}
  if p.measurement!=*mode||p.measured_lab.is_some()!=lab||p.spectrum.is_some()!=spectra{return Err("Mixed measurement modes or missing measurement fields; use CSV".into())}
  write!(out,"{}",quote(&p.id)?).unwrap();
  for v in &p.inks{write!(out," {v}").unwrap();}
  if let Some(l)=p.measured_lab{
   if p.measurement_basis!="Declared D50/2°"||l.iter().any(|x|!x.is_finite())||!(0.0..=100.).contains(&l[0]){return Err("Stored Lab is not declared D50/2° or is invalid; use CSV".into())}
   for v in l{write!(out," {v}").unwrap();}
  }
  if let Some((start,step,count))=grid{
   let s=p.spectrum.as_ref().ok_or("Missing reflectance")?;
   if p.start_nm!=Some(start)||p.step_nm!=Some(step)||s.len()!=count||s.iter().any(|v|!v.is_finite()||*v<0.){return Err("Mixed or invalid spectral grids; use CSV".into())}
   for v in s{write!(out," {v}").unwrap();}
  }
  out.push('\n');
  if out.len()>32*1024*1024-16{return Err("CGATS exceeds 32 MiB".into())}
 }
 out.push_str("END_DATA\n");Ok(out)
}


#[cfg(test)]mod tests{
 use super::*;
 #[test]fn export_roundtrip_never_writes_predictions(){
  let mut c=read("CGATS.17\nMEASUREMENT_CONDITION M0\nSPECTRAL_NORM 100\nBEGIN_DATA_FORMAT\nSAMPLE_ID CH1 CH2 CH3 SPEC_380 SPEC_390\nEND_DATA_FORMAT\nBEGIN_DATA\n\"Patch 1\" 0 50 25 30 70\nEND_DATA\n",3,false).unwrap();
  c.patches[0].predicted_lab=[42.,0.,0.];c.patches[0].comparison_lab=Some([50.,0.,0.]);
  let s=write(&c,3,false).unwrap();assert!(!s.contains("LAB_L"));
  let d=read(&s,3,false).unwrap();assert_eq!(d.patches[0].inks,c.patches[0].inks);assert_eq!(d.patches[0].spectrum,c.patches[0].spectrum);assert_eq!(d.patches[0].id,"Patch 1");assert_eq!(d.patches[0].measurement,"M0");
 }
 #[test]fn export_declared_measured_lab(){
  let c=read("CTI3\nILLUMINANT D50\nOBSERVER 2\nBEGIN_DATA_FORMAT\nSAMPLE_ID CMYK_C CMYK_M CMYK_Y CMYK_K LAB_L LAB_A LAB_B\nEND_DATA_FORMAT\nBEGIN_DATA\np1 0 50 25 0 42 -3 12\nEND_DATA\n",4,true).unwrap();
  let d=read(&write(&c,4,true).unwrap(),4,true).unwrap();assert_eq!(d.patches[0].measured_lab,Some([42.,-3.,12.]));
  let mut c=c;c.patches[0].measurement_basis="Unknown".into();assert!(write(&c,4,true).is_err());
 }
 #[test]fn export_rejects_missing_or_mixed_measurements(){
  let mut c=read("CGATS.17\nSPECTRAL_NORM 1\nBEGIN_DATA_FORMAT\nSAMPLE_ID CH1 CH2 SPEC_380 SPEC_390\nEND_DATA_FORMAT\nBEGIN_DATA\np1 0 50 0.3 0.7\np2 5 10 0.4 0.8\nEND_DATA\n",2,false).unwrap();
  c.patches[1].spectrum=None;assert!(write(&c,2,false).is_err());
  c.patches[1].spectrum=Some(vec![0.4,0.8]);c.patches[1].step_nm=Some(20);assert!(write(&c,2,false).is_err());
 }
 #[test]fn measured_and_quoted_ids(){
  let c=read("CTI3\nILLUMINANT D50\nOBSERVER 2\nNUMBER_OF_FIELDS 8\nNUMBER_OF_SETS 1\nBEGIN_DATA_FORMAT\nSAMPLE_ID CMYK_C CMYK_M CMYK_Y CMYK_K LAB_L LAB_A LAB_B\nEND_DATA_FORMAT\nBEGIN_DATA\n\"Patch 1\" 0 50 25 0 42 -3 12\nEND_DATA\n",4,true).unwrap();
  assert_eq!(c.patches[0].id,"Patch 1");assert_eq!(c.patches[0].measured_lab,Some([42.,-3.,12.]));assert!(!c.original_layout);
 }
 #[test]fn iso28178_process_colors_and_generated_row_ids(){
  let names=["Cyan","Magenta","Yellow","Black","Orange","Green","Violet"].map(str::to_owned);
  let mut s=String::from("ISO28178\nILLUMINATION_NAME D50\nOBSERVER_ANGLE 2\nFILTER M1\n");
  for (i,name) in names.iter().enumerate(){s.push_str(&format!("PROCESSCOLOR_ID \"7 {} {name}\"\n",i+1));}
  s.push_str("NUMBER_OF_FIELDS 10\nBEGIN_DATA_FORMAT\nPC7_1 PC7_2 PC7_3 PC7_4 PC7_5 PC7_6 PC7_7 LAB_L LAB_A LAB_B\nEND_DATA_FORMAT\nNUMBER_OF_SETS 2\nBEGIN_DATA\n0 0 0 0 0 0 0 92 1 -3\n0 0 0 10 0 0 0 80 0 0\nEND_DATA\n");
  let c=read_with_names(&s,7,false,Some(&names)).unwrap();assert_eq!(c.patches.len(),2);assert_eq!(c.patches[0].id,"row-000001");assert_eq!(c.patches[1].inks[3],10.);assert_eq!(c.patches[0].measurement,"M1");assert_eq!(c.patches[0].measurement_basis,"Declared D50/2°");
  assert!(read(&s,7,false).is_err());let mut wrong=names.clone();wrong.swap(3,4);assert!(read_with_names(&s,7,false,Some(&wrong)).is_err());
  let mut vendor=String::from("ISO28178\nILLUMINATION_NAME D50\nOBSERVER_ANGLE 2\nFILTER M1\n");
  for (i,name) in names.iter().enumerate(){vendor.push_str(&format!("LGOMCCHANNEL{:02} \"InkName = '{name}' LabFullToneColor = '50 0 0'\"\n",i+1));}
  vendor.push_str("NUMBER_OF_FIELDS 11\nBEGIN_DATA_FORMAT\nSAMPLE_ID PC7_1 PC7_2 PC7_3 PC7_4 PC7_5 PC7_6 PC7_7 LAB_L LAB_A LAB_B\nEND_DATA_FORMAT\nNUMBER_OF_SETS 1\nBEGIN_DATA\np1 0 0 0 0 0 0 0 92 1 -3\nEND_DATA\n");
  let c=read_with_names(&vendor,7,false,Some(&names)).unwrap();assert_eq!(c.patches[0].id,"p1");assert!(read_with_names(&vendor.replace("InkName = 'Black'","InkName = 'Blue'"),7,false,Some(&names)).is_err());
 }
 #[test]fn refuse_conflicting_metadata(){
  assert!(read("CGATS.17\nILLUMINANT D50\nILLUMINANT D65\nBEGIN_DATA_FORMAT\nSAMPLE_ID CH1 CH2\nEND_DATA_FORMAT\nBEGIN_DATA\np 0 0\nEND_DATA\n",2,false).is_err());
 }
 #[test]fn refuse_duplicate_fields(){assert!(read("CGATS.17\nBEGIN_DATA_FORMAT\nSAMPLE_ID SAMPLE_ID\nEND_DATA_FORMAT\nBEGIN_DATA\na b\nEND_DATA",4,true).is_err());}
}
