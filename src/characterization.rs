//! Vendor-neutral characterization explorer. Embedded measured data and
//! ICC-predicted Lab must remain distinct. CxF3 target/measurement adapter.
use crate::{Profile,R};
use std::{collections::BTreeMap,io::Read,path::Path};
use lcms2::{Profile as Native,Transform,Intent,Flags,PixelFormat,CIEXYZ,CIEXYZExt,CIELab,CIELabExt};
const MAX_XML:u64=32*1024*1024;
#[derive(Clone,Debug)]
pub struct Patch{
 pub id:String,pub page:usize,pub row:usize,pub column:usize,pub inks:Vec<f64>,
 pub spectrum:Option<Vec<f64>>,pub start_nm:Option<u32>,pub step_nm:Option<u32>,pub measurement:String,
 pub predicted_lab:[f64;3],pub measured_lab:Option<[f64;3]>,pub absolute_lab:Option<[f64;3]>,pub comparison_lab:Option<[f64;3]>,pub delta_e:Option<f64>,pub measurement_basis:String
}
#[derive(Clone,Debug)]
pub struct Chart{pub source:String,pub original_layout:bool,pub patches:Vec<Patch>}
fn name(n:roxmltree::Node<'_,'_>,s:&str)->bool{n.is_element()&&n.tag_name().name()==s}
fn field<'a,'b>(n:roxmltree::Node<'a,'b>,s:&str)->Option<&'a str>{n.descendants().find(|x|name(*x,s)).and_then(|x|x.text())}
fn number(s:&str)->R<f64>{let v=s.trim().parse::<f64>().map_err(|_|"Invalid numeric patch data")?;if !v.is_finite(){return Err("Nonfinite patch data".into())}Ok(v)}
fn values(n:roxmltree::Node<'_,'_>,keys:&[&str])->R<Vec<f64>>{keys.iter().map(|k|number(field(n,k).ok_or("Missing color component")?)).collect()}
fn location(n:roxmltree::Node<'_,'_>)->R<(usize,usize,usize,String)>{
 let tag=|key:&str|n.descendants().find(|x|name(*x,"Tag")&&x.attribute("Name")==Some(key)).and_then(|x|x.attribute("Value")).ok_or("Missing location tag");
 let index=|k|->R<usize>{let i=tag(k)?.parse::<usize>().map_err(|_|"Invalid location index")?;if i>10000{return Err("Location exceeds chart limit".into())}Ok(i)};
 Ok((index("Page")?,index("Row")?,index("Column")?,tag("SampleID")?.to_owned()))
}
fn cxf(t:&[u8],n:usize)->R<Chart>{
 let decoded=if t.starts_with(b"ZXML"){
  if t.len()<12{return Err("Truncated ZXML".into())}
  let mut buf=Vec::new();flate2::read::ZlibDecoder::new(&t[12..]).take(MAX_XML+1).read_to_end(&mut buf).map_err(|e|e.to_string())?;
  if buf.len() as u64>MAX_XML{return Err("CxF decompression limit exceeded".into())}buf
 }else if t.starts_with(b"<?xml")||t.starts_with(b"<"){t.to_vec()}
 else{return Err("Unrecognized CxF tag encoding".into())};
 let xml=std::str::from_utf8(&decoded).map_err(|e|e.to_string())?.trim_end_matches('\0').trim();
 let doc=roxmltree::Document::parse(xml).map_err(|e|e.to_string())?;
 if doc.root_element().tag_name().name()!="CxF"{return Err("Not a CxF document".into())}
 let mut target=BTreeMap::new();let mut measured=BTreeMap::new();
 let mut steps=BTreeMap::new();
 for spec in doc.descendants().filter(|x|name(*x,"ColorSpecification")){
  if let Some(id)=spec.attribute("Id"){if let Some(r)=spec.descendants().find(|x|name(*x,"WavelengthRange")){
   if let Some(step)=r.attribute("Increment").and_then(|x|x.parse::<u32>().ok()).filter(|&x|x>0){steps.insert(id,step);}
  }}
 }
 for obj in doc.descendants().filter(|x|name(*x,"Object")){
  let kind=obj.attribute("ObjectType").unwrap_or("");
  if kind=="Target"{
   let (page,row,col,id)=location(obj)?;
   let dev=obj.descendants().find(|x|name(*x,"ColorCMYK")).ok_or("Unsupported CxF target device space")?;
   if n!=4{return Err("CxF CMYK target does not match ICC channel count".into())}
   let inks=values(dev,&["Cyan","Magenta","Yellow","Black"])?;
   if inks.iter().any(|&v|!(0.0..=100.).contains(&v)){return Err("CxF device percentage outside range".into())}
   let key=(page,row,col,id.clone());
   if target.insert(key,Patch{id,page,row,column:col,inks,spectrum:None,start_nm:None,step_nm:None,measurement:String::new(),predicted_lab:[0.;3],measured_lab:None,absolute_lab:None,comparison_lab:None,delta_e:None,measurement_basis:String::new()}).is_some(){return Err("Duplicate CxF target location".into())}
  }else if kind=="M0_Measurement"||kind=="Measurement"{
   let key=location(obj)?;
   let Some(s)=obj.descendants().find(|x|name(*x,"ReflectanceSpectrum"))else{continue};
   let spectrum=s.text().ok_or("Empty spectrum")?.split_whitespace().map(number).collect::<R<Vec<_>>>()?;
   if spectrum.is_empty()||spectrum.len()>1024||spectrum.iter().any(|&x|x<0.){return Err("Invalid reflectance spectrum".into())}
   let start=s.attribute("StartWL").and_then(|x|x.parse::<u32>().ok());
   let step=s.attribute("ColorSpecification").and_then(|id|steps.get(id)).copied();
   if measured.insert(key,(spectrum,start,step,kind.to_owned())).is_some(){return Err("Duplicate CxF measurement location".into())}
  }
  if target.len()>100000||measured.len()>100000{return Err("Chart patch limit exceeded".into())}
 }
 if target.is_empty(){return Err("No supported CxF target records".into())}
 if measured.keys().any(|k|!target.contains_key(k)){return Err("Measurement has no matching target location and ID".into())}
 let mut positions=std::collections::HashSet::new();
 let mut patches=Vec::new();
 for (key,mut p) in target{
  if !positions.insert((p.page,p.row,p.column)){return Err("Duplicate physical patch position".into())}
  if let Some((v,start,step,mode))=measured.remove(&key){p.spectrum=Some(v);p.start_nm=start;p.step_nm=step;p.measurement=mode;}
  patches.push(p);
 }
 Ok(Chart{source:"Embedded CxF (original target)".into(),original_layout:true,patches})
}
fn predict(p:&Profile,chart:&mut Chart)->R<()>{
 let native=Native::new_icc(&p.bytes()?).map_err(|e|e.to_string())?;let f=crate::print_space::for_profile(&native)?;
 let xyz=Native::new_xyz();
 let transform=Transform::<u8,[f64;3]>::new_multiprofile(&[&native,&xyz],f.pixel,PixelFormat::XYZ_DBL,Intent::RelativeColorimetric,Flags::NO_OPTIMIZE).map_err(|e|e.to_string())?;
 let mut data=Vec::with_capacity(chart.patches.len()*f.channels*2);
 for p in &chart.patches{if p.inks.len()!=f.channels{return Err("Patch channel count mismatch".into())}for &v in &p.inks{data.extend_from_slice(&((v/100.*65535.).round() as u16).to_ne_bytes());}}
 let mut out=vec![[0.;3];chart.patches.len()];transform.transform_pixels(&data,&mut out);
 let white=CIEXYZ{X:0.9642,Y:1.,Z:0.8249};
 for (p,x) in chart.patches.iter_mut().zip(out){let l=CIEXYZ{X:x[0],Y:x[1],Z:x[2]}.to_lab(&white);
  if [l.L,l.a,l.b].iter().any(|v|!v.is_finite()){return Err("Invalid predicted Lab".into())}p.predicted_lab=[l.L,l.a,l.b];
 }
 // Absolute prediction uses the same D50 perfect-diffuser reference as
 // spectral/absolute measurements. Keep relative preview unchanged.
 if let Ok(t)=Transform::<u8,[f64;3]>::new_multiprofile(&[&native,&xyz],f.pixel,PixelFormat::XYZ_DBL,Intent::AbsoluteColorimetric,Flags::NO_OPTIMIZE){
  let mut absolute=vec![[0.;3];chart.patches.len()];t.transform_pixels(&data,&mut absolute);
  for (patch,v) in chart.patches.iter_mut().zip(absolute){
   let l=CIEXYZ{X:v[0],Y:v[1],Z:v[2]}.to_lab(&white);
   if [l.L,l.a,l.b].iter().all(|v|v.is_finite()){patch.absolute_lab=Some([l.L,l.a,l.b]);}
  }
 }
 Ok(())
}
fn prepare_measurements(c:&mut Chart){
 for p in &mut c.patches{
  if p.measurement_basis=="Declared D50/2°"{
   p.comparison_lab=p.measured_lab;
  }
  if p.comparison_lab.is_none(){
   if let (Some(s),Some(start),Some(step))=(&p.spectrum,p.start_nm,p.step_nm){
    use crate::cie_spectral::{self,Illuminant,Observer};
    if cie_spectral::available(){
     if let Ok(r)=cie_spectral::calculate(s,start,step,Illuminant::D50,Observer::Two){
      p.comparison_lab=Some(r.lab);
      p.measurement_basis=format!("Spectral estimate: CIE D50/2° 1 nm integration; {}endpoints; original measurement mode retained",if r.endpoint_extended{"extended "}else{"measured "});
     }
    }else if let Ok(l)=crate::spectral_colorimetry::lab(s,start,step){
     p.comparison_lab=Some(l);p.measurement_basis="Spectral estimate: legacy D50/2° (T.42); official tables unavailable".into();
    }
   }
  }
 }
}
#[derive(Clone,Debug)]
pub struct Fit{pub count:usize,pub mean:f64,pub p95:f64,pub max:f64}
/// Only call after operator explicitly confirms comparable measurement
/// conditions and absolute reference. This is characterization fit,
// not independent validation and not proof of M0/M1 equivalence.
pub fn compare_absolute(c:&mut Chart)->R<Fit>{
 let mut ds=Vec::new();
 for p in &mut c.patches{
  p.delta_e=None;
  if let (Some(m),Some(a))=(p.comparison_lab,p.absolute_lab){
   let to=|v:[f64;3]|CIELab{L:v[0],a:v[1],b:v[2]};
   let d=to(m).cie2000_delta_e(&to(a),1.,1.,1.);
   if !d.is_finite()||d<0.{return Err("Invalid characterization Delta E".into())}
   p.delta_e=Some(d);ds.push(d);
  }
 }
 if ds.is_empty(){return Err("No comparable absolute D50/2° pairs; measured Lab conditions or spectral grid may be unsupported".into())}
 ds.sort_by(f64::total_cmp);let count=ds.len();let p95=ds[((count as f64*0.95).ceil() as usize).saturating_sub(1)];
 Ok(Fit{count,mean:ds.iter().sum::<f64>()/count as f64,p95,max:ds[count-1]})
}
pub fn extract(p:&Profile)->R<Chart>{
 let n=crate::print_space::format(&p.header[16..20])?.channels;
 let mut errors=Vec::new();
 if let Some(t)=p.tags.get(b"CxF "){
  match cxf(t,n){Ok(mut c)=>{predict(p,&mut c)?;prepare_measurements(&mut c);return Ok(c)},Err(e)=>errors.push(format!("CxF: {e}"))}
 }
 if let Some(t)=p.tags.get(b"targ"){
  let result=(||->R<Chart>{
   if !t.starts_with(b"text")||t.len()<8{return Err("Unsupported targ tag encoding".into())}
   let s=std::str::from_utf8(&t[8..]).map_err(|e|e.to_string())?.trim_end_matches('\0');
   let mut c=crate::characterization_cgats::read(s,n,p.cmyk())?;
   c.source="Embedded targ / CGATS".into();Ok(c)
  })();
  match result{Ok(mut c)=>{predict(p,&mut c)?;prepare_measurements(&mut c);return Ok(c)},Err(e)=>errors.push(format!("targ: {e}"))}
 }
 Err(if errors.is_empty(){"No supported embedded target; generate a virtual chart or import CGATS".into()}else{errors.join("; ")})
}
pub fn import_cgats(p:&Profile,path:&Path)->R<Chart>{
 let n=crate::print_space::format(&p.header[16..20])?.channels;
 if std::fs::metadata(path).map_err(|e|e.to_string())?.len()>MAX_XML{return Err("CGATS exceeds 32 MiB".into())}
 let s=std::fs::read_to_string(path).map_err(|e|e.to_string())?;
 let mut c=crate::characterization_cgats::read(&s,n,p.cmyk())?;
 predict(p,&mut c)?;prepare_measurements(&mut c);Ok(c)
}
/// Prefer the actual embedded target; a failed or absent target falls back
/// to an explicitly model-predicted virtual chart. Preserve the diagnostic.
pub fn automatic_chart(p:&Profile)->R<(Chart,Option<String>)>{
 match extract(p){
  Ok(c)=>Ok((c,None)),
  Err(e)=>virtual_chart(p).map(|c|(c,Some(e.clone()))).map_err(|v|format!("{e}; virtual target: {v}")),
 }
}
pub fn virtual_chart(p:&Profile)->R<Chart>{
 let n=crate::print_space::format(&p.header[16..20])?.channels;
 let mut rows=vec![vec![0.;n]];
 for j in 0..n{for i in 1..=10{let mut v=vec![0.;n];v[j]=i as f64*10.;rows.push(v);}}
 for j in 0..n{for k in j+1..n{for a in [25.,50.,75.,100.]{for b in [25.,50.,75.,100.]{let mut v=vec![0.;n];v[j]=a;v[k]=b;rows.push(v);}}}}
 let mut c=Chart{source:"ICC-predicted virtual target (not measured)".into(),original_layout:false,patches:rows.into_iter().enumerate().map(|(i,inks)|Patch{id:(i+1).to_string(),page:1+i/400,row:(i%400)/20,column:i%20,inks,spectrum:None,start_nm:None,step_nm:None,measurement:String::new(),predicted_lab:[0.;3],measured_lab:None,absolute_lab:None,comparison_lab:None,delta_e:None,measurement_basis:String::new()}).collect()};
 predict(p,&mut c)?;Ok(c)
}
pub fn export_csv(c:&Chart,path:&Path)->R<()>{
 let file=std::fs::OpenOptions::new().write(true).create_new(true).open(path).map_err(|e|e.to_string())?;
 let mut w=csv::Writer::from_writer(file);
 let n=c.patches.first().map(|p|p.inks.len()).unwrap_or(0);
 let mut header=["Source","SampleID","Page","Row","Column","Measurement","Start_nm","Step_nm","Reflectance","Predicted_L","Predicted_a","Predicted_b","Absolute_L","Absolute_a","Absolute_b","Stored_Measured_L","Stored_Measured_a","Stored_Measured_b","Comparison_L","Comparison_a","Comparison_b","Measurement_Basis","DeltaE00"].map(str::to_owned).to_vec();
 header.extend((0..n).map(|j|format!("CH{}",j+1)));w.write_record(header).map_err(|e|e.to_string())?;
 for p in &c.patches{
  let mut r=vec![c.source.clone(),p.id.clone(),p.page.to_string(),p.row.to_string(),p.column.to_string(),p.measurement.clone(),p.start_nm.map(|x|x.to_string()).unwrap_or_default(),p.step_nm.map(|x|x.to_string()).unwrap_or_default(),p.spectrum.as_ref().map(|v|v.iter().map(|x|x.to_string()).collect::<Vec<_>>().join(" ")).unwrap_or_default()];
  r.extend(p.predicted_lab.iter().map(|x|x.to_string()));
  for l in [p.absolute_lab,p.measured_lab,p.comparison_lab]{r.extend(l.map(|v|v.map(|x|x.to_string())).unwrap_or_else(||[String::new(),String::new(),String::new()]));}
  r.push(p.measurement_basis.clone());r.push(p.delta_e.map(|x|x.to_string()).unwrap_or_default());r.extend(p.inks.iter().map(|x|x.to_string()));w.write_record(r).map_err(|e|e.to_string())?;
 }w.flush().map_err(|e|e.to_string())
}
#[cfg(test)]
mod tests{
 use super::*;
 #[test]fn automatic_target_prefers_embedded_and_labels_fallback(){
  let mut p=crate::synthetic_cmyk_v4(true);
  let (c,notice)=automatic_chart(&p).unwrap();
  assert!(notice.is_some());assert!(c.source.contains("not measured"));
  assert!(!c.patches.is_empty());assert!(c.patches.iter().all(|v|v.measured_lab.is_none()));
  let mut tag=b"text\0\0\0\0".to_vec();
  tag.extend(b"CTI3\nILLUMINANT D50\nOBSERVER 2\nBEGIN_DATA_FORMAT\nSAMPLE_ID CMYK_C CMYK_M CMYK_Y CMYK_K LAB_L LAB_A LAB_B\nEND_DATA_FORMAT\nBEGIN_DATA\np1 0 0 0 0 90 0 0\nEND_DATA\n\0");
  p.tags.insert(*b"targ",tag);
  let (c,notice)=automatic_chart(&p).unwrap();assert!(notice.is_none());assert_eq!(c.patches.len(),1);assert_eq!(c.patches[0].measured_lab,Some([90.,0.,0.]));
 }
 #[test]fn embedded_cxf_pairs_by_location_not_record_order(){
  let xml=r#"<CxF><Object ObjectType="M0_Measurement"><ReflectanceSpectrum StartWL="380" ColorSpecification="m">0.1 0.2</ReflectanceSpectrum><Tag Name="Page" Value="2"/><Tag Name="Row" Value="3"/><Tag Name="Column" Value="4"/><Tag Name="SampleID" Value="7"/></Object><Object ObjectType="Target"><ColorCMYK><Cyan>20</Cyan><Magenta>0</Magenta><Yellow>50</Yellow><Black>0</Black></ColorCMYK><Tag Name="Page" Value="2"/><Tag Name="Row" Value="3"/><Tag Name="Column" Value="4"/><Tag Name="SampleID" Value="7"/></Object><ColorSpecification Id="m"><WavelengthRange Increment="10"/></ColorSpecification></CxF>"#;
  let mut raw=b"ZXML\0\0\0\0\0\0\0\x01".to_vec();
  use std::io::Write;
  let mut z=flate2::write::ZlibEncoder::new(Vec::new(),flate2::Compression::default());z.write_all(xml.as_bytes()).unwrap();z.write_all(b"\0").unwrap();raw.extend(z.finish().unwrap());
  let c=cxf(&raw,4).unwrap();assert_eq!(c.patches.len(),1);let p=&c.patches[0];assert_eq!(p.inks,vec![20.,0.,50.,0.]);assert_eq!(p.spectrum.as_deref(),Some(&[0.1,0.2][..]));assert_eq!(p.step_nm,Some(10));assert!(p.measured_lab.is_none());
 }
 #[test]fn fit_is_explicit_and_uses_absolute_not_relative(){
  let mut c=Chart{source:"test".into(),original_layout:false,patches:vec![Patch{
   id:"1".into(),page:1,row:0,column:0,inks:vec![0.;4],spectrum:Some(vec![0.5;36]),start_nm:Some(380),step_nm:Some(10),measurement:"M0".into(),predicted_lab:[99.,0.,0.],measured_lab:None,absolute_lab:Some([76.069261,0.,0.]),comparison_lab:None,delta_e:None,measurement_basis:String::new()
  }]};
  prepare_measurements(&mut c);assert!(c.patches[0].measured_lab.is_none());assert!(c.patches[0].delta_e.is_none());
  let r=compare_absolute(&mut c).unwrap();assert_eq!(r.count,1);assert!(r.max<0.0001);
 }
 #[test]fn embedded_targ_extracts_declared_measured_lab(){
  let mut p=crate::synthetic_cmyk_v4(true);
  let mut t=b"text\0\0\0\0".to_vec();
  t.extend(b"CTI3\nILLUMINANT D50\nOBSERVER 2\nBEGIN_DATA_FORMAT\nSAMPLE_ID CMYK_C CMYK_M CMYK_Y CMYK_K LAB_L LAB_A LAB_B\nEND_DATA_FORMAT\nBEGIN_DATA\n1 0 0 0 0 90 0 0\nEND_DATA\n\0");
  p.tags.insert(*b"targ",t);let c=extract(&p).unwrap();
  assert_eq!(c.patches[0].comparison_lab,Some([90.,0.,0.]));assert!(c.patches[0].delta_e.is_none());assert!(!c.original_layout);
 }
 #[test]fn virtual_is_not_measured(){let p=crate::print_space::fixture(6);let c=virtual_chart(&p).unwrap();assert!(!c.original_layout);assert!(c.patches.iter().all(|p|p.spectrum.is_none()&&p.inks.len()==6));}
}
