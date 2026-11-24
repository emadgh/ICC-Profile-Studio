//! Gradient continuity diagnostics; all predicted Lab values are evaluated by LCMS.
use crate::comparison::{open_printer,simulate};
use lcms2::{Profile,Intent,CIELabExt};
use std::path::Path;
#[derive(Clone,Debug)]
pub struct Ramp{pub name:String,pub steps:Vec<f64>,pub light:Vec<f64>,pub max_step:f64,pub p95:f64,pub mean:f64,pub worst_index:usize,pub plateaus:usize,pub spikes:usize,pub max_ink_step:f64}
#[derive(Clone,Debug)]
pub struct GradientReport{pub reference:Vec<Ramp>,pub candidate:Option<Vec<Ramp>>}
fn ramps()->Vec<(String,Vec<[u8;3]>)>{
 let mut out=Vec::new();
 let palette=[("White",[255,255,255]),("Red",[255,0,0]),("Green",[0,255,0]),("Blue",[0,0,255]),("Cyan",[0,255,255]),("Magenta",[255,0,255]),("Yellow",[255,255,0])];
 for (name,end) in palette{
  for start in [[0u8;3],[255u8;3]]{
   if start==end{continue}
   let label=if start==[0;3]{format!("Black → {name}")}else{format!("White → {name}")};
   let v=(0..256).map(|i|std::array::from_fn(|j|((start[j] as u32*(255-i)+end[j] as u32*i+127)/255) as u8)).collect();
   out.push((label,v));
  }
 }
 out
}
fn metrics(steps:&[f64])->(f64,f64,f64,usize,usize,usize){
 let mut sorted=steps.to_vec();sorted.sort_by(f64::total_cmp);
 let mean=steps.iter().sum::<f64>()/steps.len() as f64;
 let index=steps.iter().enumerate().max_by(|a,b|a.1.total_cmp(b.1)).unwrap().0;
 let spikes=steps.iter().enumerate().filter(|&(i,v)|{
  let mut local:Vec<_>=steps[i.saturating_sub(4)..(i+5).min(steps.len())].iter().enumerate().filter_map(|(j,x)|(j+i.saturating_sub(4)!=i).then_some(*x)).collect();
  local.sort_by(f64::total_cmp);!local.is_empty()&&*v>3.*local[local.len()/2]+0.5
 }).count();
 (steps[index],sorted[((steps.len() as f64*0.95).ceil() as usize)-1],mean,index,steps.iter().filter(|&&v|v<0.05).count(),spikes)
}
fn analyze(p:&Profile)->Result<Vec<Ramp>,String>{
 let mut report=Vec::new();
 for (name,values) in ramps(){
  let (inks,lab)=simulate(p,&values,Intent::RelativeColorimetric)?;
  let steps:Vec<_>=lab.windows(2).map(|v|v[0].cie2000_delta_e(&v[1],1.,1.,1.)).collect();
  if steps.iter().any(|v|!v.is_finite()||*v<0.){return Err("Invalid gradient Delta E".into())}
  let (max_step,p95,mean,worst_index,plateaus,spikes)=metrics(&steps);
  let max_ink_step=inks.windows(2).flat_map(|v|(0..4).map(move|j|v[0][j].abs_diff(v[1][j]) as f64*100./65535.)).fold(0f64,f64::max);
  report.push(Ramp{name,steps,light:lab.iter().map(|v|v.L).collect(),max_step,p95,mean,worst_index,plateaus,spikes,max_ink_step});
 }
 Ok(report)
}
pub fn compare(reference:&Path,candidate:Option<&Path>)->Result<GradientReport,String>{
 let reference=analyze(&open_printer(reference)?)?;
 let candidate=candidate.map(|p|open_printer(p).and_then(|p|analyze(&p))).transpose()?;
 Ok(GradientReport{reference,candidate})
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn ramp_endpoints(){let r=ramps();assert_eq!(r.len(),13);assert!(r.iter().all(|(_,v)|v.len()==256));assert_eq!(r[0].1[0],[0;3]);assert_eq!(r[0].1[255],[255;3]);}
 #[test]fn finds_step_and_plateau(){let mut v=vec![0.2;255];v[42]=3.;v[0]=0.;let (max,_,_,idx,plateaus,spikes)=metrics(&v);assert_eq!(max,3.);assert_eq!(idx,42);assert_eq!(plateaus,1);assert_eq!(spikes,1);}
}
