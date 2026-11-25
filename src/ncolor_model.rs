//! Experimental N-color Device -> Lab baseline, not an ICC generator.
//! Group identical recipes before holdout to avoid duplicate leakage.
//! Equal-weight Euclidean ink distance; interpolation is not a physical model.
use crate::{R,characterization::Chart};
use lcms2::{CIELab,CIELabExt};
use std::collections::BTreeMap;
const SCALE:f64=10000.; // 0.0001 percentage point
const MAX_GROUPS:usize=10000;
const K:usize=8;
#[derive(Clone,Debug)]
struct Sample{key:Vec<u32>,lab:[f64;3]}
#[derive(Clone,Debug)]
pub struct ErrorStats{pub mean:f64,pub p95:f64,pub max:f64}
#[derive(Clone,Debug)]
pub struct Report{
 pub channels:usize,pub input_rows:usize,pub unique_recipes:usize,pub duplicate_rows:usize,
 pub training_recipes:usize,pub holdout_recipes:usize,pub measurement:String,pub fold:u32,
 pub nearest:ErrorStats,pub idw:ErrorStats,pub nearest_distance:ErrorStats,
 pub max_training_tac_percent:f64,pub max_holdout_tac_percent:f64,
}
fn key(inks:&[f64],n:usize)->R<Vec<u32>>{
 if inks.len()!=n||inks.iter().any(|v|!v.is_finite()||!(0.0..=100.).contains(v)){return Err("Invalid N-color ink recipe".into())}
 Ok(inks.iter().map(|v|(v*SCALE).round() as u32).collect())
}
fn fold(key:&[u32])->u32{
 let mut h=2166136261u32;
 for q in key{for b in q.to_le_bytes(){h=(h^u32::from(b)).wrapping_mul(16777619);}}
 h%5
}
fn distance(a:&[u32],b:&[u32])->u64{
 a.iter().zip(b).map(|(&x,&y)|{let d=u64::from(x.abs_diff(y));d*d}).sum()
}
fn tac(key:&[u32])->f64{key.iter().map(|&v|f64::from(v)/SCALE).sum()}
fn stats(mut x:Vec<f64>)->R<ErrorStats>{
 if x.is_empty()||x.iter().any(|v|!v.is_finite()||*v<0.){return Err("Invalid or empty error samples".into())}
 x.sort_by(f64::total_cmp);let n=x.len();
 Ok(ErrorStats{mean:x.iter().sum::<f64>()/n as f64,p95:x[((n as f64*0.95).ceil() as usize)-1],max:x[n-1]})
}
fn de(a:[f64;3],b:[f64;3])->f64{
 let lab=|v:[f64;3]|CIELab{L:v[0],a:v[1],b:v[2]};
 lab(a).cie2000_delta_e(&lab(b),1.,1.,1.)
}
fn estimate(training:&[Sample],target:&[u32])->([f64;3],[f64;3],f64){
 // Sorted recipe keys make ties deterministic. Keep only eight neighbors.
 let mut best=[(u64::MAX,usize::MAX);K];
 for (i,s) in training.iter().enumerate(){
  let pair=(distance(&s.key,target),i);
  if pair<best[K-1]{let j=best.partition_point(|&v|v<=pair);best.copy_within(j..K-1,j+1);best[j]=pair;}
 }
 let (d,i)=best[0];let nearest=training[i].lab;
 if d==0{return (nearest,nearest,0.)}
 let mut sum=0.;let mut prediction=[0.;3];
 for &(d,i) in &best{
  if i==usize::MAX{break}
  let w=1./d as f64;sum+=w;
  for (j,v) in prediction.iter_mut().enumerate(){*v+=w*training[i].lab[j];}
 }
 for v in &mut prediction{*v/=sum;}
 (nearest,prediction,(d as f64).sqrt()/(SCALE*100.))
}
/// Analyze one of five deterministic recipe-group folds. Input must be
/// declared D50/2-degree Lab from a homogeneous measurement condition.
/// Never uses predicted_lab, spectral estimates or the original ICC.
pub fn evaluate(chart:&Chart,channels:usize,holdout_fold:u32)->R<Report>{
 if !(2..=15).contains(&channels)||holdout_fold>=5||chart.patches.is_empty()||chart.patches.len()>100000{return Err("Invalid N-color evaluation dimensions".into())}
 let mode=chart.patches[0].measurement.clone();
 if !matches!(mode.as_str(),"M0"|"M1"|"M2"|"M3"){return Err("Declare a common M0/M1/M2/M3 measurement condition".into())}
 let mut groups:BTreeMap<Vec<u32>,([f64;3],usize)>=BTreeMap::new();
 for p in &chart.patches{
  if p.measurement!=mode||p.measurement_basis!="Declared D50/2°"{return Err("N-color evaluation requires homogeneous declared D50/2° Lab".into())}
  let lab=p.measured_lab.ok_or("Missing supplied Lab data")?;
  if lab.iter().any(|v|!v.is_finite())||!(0.0..=100.).contains(&lab[0])||lab[1..].iter().any(|&v|!(-128.0..=127.0).contains(&v)){return Err("Lab outside supported PCS range".into())}
  let entry=groups.entry(key(&p.inks,channels)?).or_insert(([0.;3],0));entry.1+=1;
  for j in 0..3{entry.0[j]+=lab[j];}
  if groups.len()>MAX_GROUPS{return Err("N-color baseline exceeds 10,000 unique recipes".into())}
 }
 let unique=groups.len();let mut training=Vec::new();let mut holdout=Vec::new();
 for (key,(sum,n)) in groups{let sample=Sample{key,lab:sum.map(|v|v/n as f64)};if fold(&sample.key)==holdout_fold{holdout.push(sample)}else{training.push(sample)}}
 if training.len()<K||holdout.len()<2{return Err("Insufficient unique training or holdout recipes".into())}
 let mut nearest=Vec::with_capacity(holdout.len());let mut idw=nearest.clone();let mut distances=nearest.clone();
 for p in &holdout{let (a,b,d)=estimate(&training,&p.key);nearest.push(de(p.lab,a));idw.push(de(p.lab,b));distances.push(d);}
 Ok(Report{channels,input_rows:chart.patches.len(),unique_recipes:unique,duplicate_rows:chart.patches.len()-unique,
 training_recipes:training.len(),holdout_recipes:holdout.len(),measurement:mode,fold:holdout_fold,
 nearest:stats(nearest)?,idw:stats(idw)?,nearest_distance:stats(distances)?,
 max_training_tac_percent:training.iter().map(|p|tac(&p.key)).fold(0.,f64::max),
 max_holdout_tac_percent:holdout.iter().map(|p|tac(&p.key)).fold(0.,f64::max)})
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn recipe_groups_prevent_leakage_and_predictions_are_ignored(){
  let mut s=String::from("CTI3\nILLUMINANT D50\nOBSERVER 2\nMEASUREMENT_CONDITION M1\nBEGIN_DATA_FORMAT\nSAMPLE_ID CH1 CH2 CH3 LAB_L LAB_A LAB_B\nEND_DATA_FORMAT\nBEGIN_DATA\n");
  for i in 0..=20{for j in 0..=5{for repeat in 0..2{
   s.push_str(&format!("p-{i}-{j}-{repeat} {} {} 0 {} {} 0\n",i*5,j*20,90.-i as f64*2.,j*3));
  }}}
  s.push_str("END_DATA\n");let mut c=crate::characterization_cgats::read(&s,3,false).unwrap();
  let r=evaluate(&c,3,0).unwrap();assert_eq!(r.unique_recipes,126);assert_eq!(r.duplicate_rows,126);assert_eq!(r.training_recipes+r.holdout_recipes,126);
  for p in &mut c.patches{p.predicted_lab=[0.,100.,100.];}
  let q=evaluate(&c,3,0).unwrap();assert_eq!(r.idw.mean,q.idw.mean);
  c.patches[0].measurement="M0".into();assert!(evaluate(&c,3,0).is_err());
 }
 #[test]fn known_exact_and_tie_order(){
  let training=(0..8).map(|i|Sample{key:vec![i*100,0],lab:[i as f64,0.,0.]}).collect::<Vec<_>>();
  let (a,b,d)=estimate(&training,&[100,0]);assert_eq!(a,b);assert_eq!(a,[1.,0.,0.]);assert_eq!(d,0.);
  let (a,_,_)=estimate(&training,&[150,0]);assert_eq!(a,[1.,0.,0.]);
 }
 #[test]#[ignore="Requires operator-supplied FOGRA55 characterization data"]
 fn fogra55_ncolor_baseline(){
  let path=std::env::var_os("ICC_FOGRA55_DATA").expect("Set ICC_FOGRA55_DATA");
  let text=std::fs::read_to_string(path).unwrap();
  let names=["Cyan","Magenta","Yellow","Black","Orange","Green","Violet"].map(str::to_owned);
  let c=crate::characterization_cgats::read_with_names(&text,7,false,Some(&names)).unwrap();
  let r=evaluate(&c,7,0).unwrap();println!("{r:#?}");
  assert_eq!(r.input_rows,4884);assert_eq!(r.unique_recipes,4272);assert_eq!(r.training_recipes,3405);assert_eq!(r.holdout_recipes,867);
 }
}
