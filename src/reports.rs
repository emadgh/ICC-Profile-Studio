//! CSV exports for numerical analysis, without mixing simulated and measured results.
use std::{path::Path,io::Write};
type R<T>=Result<T,String>;
fn err(e:impl std::fmt::Display)->String{e.to_string()}
fn save(path:&Path,write:impl FnOnce(&mut csv::Writer<&mut std::fs::File>)->R<()>)->R<()>{
 let mut f=tempfile::NamedTempFile::new_in(path.parent().unwrap_or(Path::new("."))).map_err(err)?;
 {let mut w=csv::Writer::from_writer(f.as_file_mut());write(&mut w)?;w.flush().map_err(err)?;}
 f.as_file_mut().flush().map_err(err)?;f.persist_noclobber(path).map_err(err)?;Ok(())
}
pub fn comparison(path:&Path,r:&crate::comparison::Comparison)->R<()>{
 save(path,|w|{
  w.write_record(["metric","index","before","after","difference"]).map_err(err)?;
  w.write_record(["separation_intent".to_owned(),format!("{:?}",r.intent),String::new(),String::new(),String::new()]).map_err(err)?;
  for (name,v) in [("sample_count",r.count as f64),("mean_delta_e00",r.mean_de),("p95_delta_e00",r.p95_de),("max_delta_e00",r.max_de)]{
   w.write_record([name.to_owned(),String::new(),String::new(),String::new(),v.to_string()]).map_err(err)?;
  }
  // No inferred channel pairing, even when channel counts are equal.
  for (side,stats,channels) in [(0,&r.reference,&r.reference_channels),(1,&r.candidate,&r.candidate_channels)]{
   for (j,ch) in channels.iter().enumerate(){
    for (metric,value) in [("mean_ink_percent",stats.mean[j]),("max_ink_percent",stats.max[j])]{
     let index=format!("{}:{}:{}",if side==0{"reference"}else{"candidate"},j+1,ch.name);
     let v=value.to_string();
     let (a,b)=if side==0{(v,String::new())}else{(String::new(),v)};
     w.write_record([metric.into(),index,a,b,String::new()]).map_err(err)?;
    }
   }
  }
  for (name,a,b) in [("mean_tac",r.reference.mean_tac,r.candidate.mean_tac),("max_sampled_tac",r.reference.max_tac,r.candidate.max_tac)]{w.write_record([name.into(),String::new(),a.to_string(),b.to_string(),(b-a).to_string()]).map_err(err)?;}
  for n in &r.neutral{
   w.write_record(["neutral_delta_e00".into(),n.rgb.to_string(),String::new(),String::new(),n.delta_e.to_string()]).map_err(err)?;
   for (side,values) in [(0,&n.reference_ink),(1,&n.candidate_ink)]{
    for (j,value) in values.iter().enumerate(){
     let v=value.to_string();let (a,b)=if side==0{(v,String::new())}else{(String::new(),v)};
     w.write_record([format!("neutral_{}_ink_{}",if side==0{"reference"}else{"candidate"},j+1),n.rgb.to_string(),a,b,String::new()]).map_err(err)?;
    }
   }
   for (j,name) in ["neutral_L","neutral_a","neutral_b"].into_iter().enumerate(){let a=n.reference_lab[j];let b=n.candidate_lab[j];w.write_record([name.into(),n.rgb.to_string(),a.to_string(),b.to_string(),(b-a).to_string()]).map_err(err)?;}
  }
  Ok(())
 })
}
pub fn gradient(path:&Path,r:&crate::gradient::GradientReport)->R<()>{
 save(path,|w|{
  let mut header=vec!["ramp".into(),"input_index".into(),"before_delta_e00_to_next".into(),"after_delta_e00_to_next".into(),"before_L".into(),"after_L".into()];
  for (side,channels) in [("before",&r.reference_channels),("after",&r.candidate_channels)]{
   for (j,ch) in channels.iter().enumerate(){header.push(format!("{side}_ink_{}:{}",j+1,ch.name));}
  }
  w.write_record(header).map_err(err)?;
  for (i,a) in r.reference.iter().enumerate(){
   let b=r.candidate.as_ref().and_then(|v|v.get(i));
   for j in 0..a.light.len(){
    let get=|v:&[f64]|v.get(j).map(|x|x.to_string()).unwrap_or_default();
    let mut row=vec![a.name.clone(),j.to_string(),get(&a.steps),b.map(|v|get(&v.steps)).unwrap_or_default(),get(&a.light),b.map(|v|get(&v.light)).unwrap_or_default()];
    for curve in &a.ink_curves{row.push(get(curve));}
    if let Some(b)=b{for curve in &b.ink_curves{row.push(get(curve));}}
    else{row.extend(std::iter::repeat_n(String::new(),r.candidate_channels.len()));}
    w.write_record(row).map_err(err)?;
   }
  }
  Ok(())
 })
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn mixed_channel_exports(){
  let d=tempfile::tempdir().unwrap();let a=d.path().join("a.icc");let b=d.path().join("b.icc");
  crate::synthetic_cmyk_v4(true).save_new(&a).unwrap();crate::print_space::fixture(6).save_new(&b).unwrap();
  let p=d.path().join("cmp.csv");let c=crate::comparison::compare(&a,&b,9,lcms2::Intent::RelativeColorimetric).unwrap();
  comparison(&p,&c).unwrap();let data=std::fs::read_to_string(p).unwrap();assert!(data.contains("candidate:6:Channel 6"));assert!(data.contains("neutral_candidate_ink_6"));
  let p=d.path().join("gradient.csv");let g=crate::gradient::compare(&a,Some(&b)).unwrap();gradient(&p,&g).unwrap();
  let mut csv=csv::Reader::from_path(p).unwrap();assert_eq!(csv.headers().unwrap().len(),16);assert_eq!(csv.records().collect::<Result<Vec<_>,_>>().unwrap().len(),13*256);
 }
}
