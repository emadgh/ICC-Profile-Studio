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
  for j in 0..4{for (name,a,b) in [("mean_ink_percent",r.reference.mean[j],r.candidate.mean[j]),("max_ink_percent",r.reference.max[j],r.candidate.max[j])]{
   w.write_record([name.to_owned(),(j+1).to_string(),a.to_string(),b.to_string(),(b-a).to_string()]).map_err(err)?;
  }}
  for (name,a,b) in [("mean_tac",r.reference.mean_tac,r.candidate.mean_tac),("max_sampled_tac",r.reference.max_tac,r.candidate.max_tac)]{w.write_record([name.into(),String::new(),a.to_string(),b.to_string(),(b-a).to_string()]).map_err(err)?;}
  for n in &r.neutral{
   w.write_record(["neutral_delta_e00".into(),n.rgb.to_string(),String::new(),String::new(),n.delta_e.to_string()]).map_err(err)?;
   for j in 0..4{let a=n.reference_ink[j];let b=n.candidate_ink[j];w.write_record([format!("neutral_ink_{}",j+1),n.rgb.to_string(),a.to_string(),b.to_string(),(b-a).to_string()]).map_err(err)?;}
   for (j,name) in ["neutral_L","neutral_a","neutral_b"].into_iter().enumerate(){let a=n.reference_lab[j];let b=n.candidate_lab[j];w.write_record([name.into(),n.rgb.to_string(),a.to_string(),b.to_string(),(b-a).to_string()]).map_err(err)?;}
  }
  Ok(())
 })
}
pub fn gradient(path:&Path,r:&crate::gradient::GradientReport)->R<()>{
 save(path,|w|{
  w.write_record(["ramp","adjacent_step","before_delta_e00","after_delta_e00"]).map_err(err)?;
  for (i,a) in r.reference.iter().enumerate(){let b=r.candidate.as_ref().and_then(|v|v.get(i));for (j,de) in a.steps.iter().enumerate(){w.write_record([a.name.clone(),j.to_string(),de.to_string(),b.and_then(|v|v.steps.get(j)).map(|v|v.to_string()).unwrap_or_default()]).map_err(err)?;}}Ok(())
 })
}
