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
fn grouped(chart:&Chart,channels:usize)->R<(String,Vec<Sample>)>{
 if !(2..=15).contains(&channels)||chart.patches.is_empty()||chart.patches.len()>100000{return Err("Invalid N-color evaluation dimensions".into())}
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
 Ok((mode,groups.into_iter().map(|(key,(sum,n))|Sample{key,lab:sum.map(|v|v/n as f64)}).collect()))
}
pub fn evaluate(chart:&Chart,channels:usize,holdout_fold:u32)->R<Report>{
 if holdout_fold>=5{return Err("Invalid holdout fold".into())}
 let (mode,groups)=grouped(chart,channels)?;
 let unique=groups.len();let (holdout,training):(Vec<_>,Vec<_>)=groups.into_iter().partition(|p|fold(&p.key)==holdout_fold);
 if training.len()<K||holdout.len()<2{return Err("Insufficient unique training or holdout recipes".into())}
 let mut nearest=Vec::with_capacity(holdout.len());let mut idw=nearest.clone();let mut distances=nearest.clone();
 for p in &holdout{let (a,b,d)=estimate(&training,&p.key);nearest.push(de(p.lab,a));idw.push(de(p.lab,b));distances.push(d);}
 Ok(Report{channels,input_rows:chart.patches.len(),unique_recipes:unique,duplicate_rows:chart.patches.len()-unique,
 training_recipes:training.len(),holdout_recipes:holdout.len(),measurement:mode,fold:holdout_fold,
 nearest:stats(nearest)?,idw:stats(idw)?,nearest_distance:stats(distances)?,
 max_training_tac_percent:training.iter().map(|p|tac(&p.key)).fold(0.,f64::max),
 max_holdout_tac_percent:holdout.iter().map(|p|tac(&p.key)).fold(0.,f64::max)})
}
// Local weighted affine regression centred on the query. Ridge on slopes,
// not intercept, controls poorly supported axes. No new dependency.
const LOCAL_K:usize=32;
const RIDGE:f64=0.002;
#[derive(Clone,Debug)]
pub struct Validation{pub train:usize,pub test:usize,pub nearest:ErrorStats,pub idw:ErrorStats,pub local:ErrorStats,pub nearest_distance:ErrorStats}
#[derive(Clone,Debug)]
pub struct CrossValidation{pub channels:usize,pub input_rows:usize,pub unique_recipes:usize,pub duplicate_rows:usize,pub measurement:String,pub folds:Vec<Validation>,pub pooled:Validation,pub stress:Option<Validation>,pub active_counts:Vec<usize>,pub peak_tac_percent:f64}
fn local_prediction(training:&[&Sample],target:&[u32])->R<([f64;3],[f64;3],[f64;3],f64)>{local_kernel(training,target,false)}
fn local_kernel(training:&[&Sample],target:&[u32],tapered:bool)->R<([f64;3],[f64;3],[f64;3],f64)>{
 let count=LOCAL_K+if tapered{1}else{0};
 if training.len()<count{return Err("Insufficient neighbors for local model".into())}
 let mut best=[(u64::MAX,usize::MAX);LOCAL_K+1];
 for (i,s) in training.iter().enumerate(){
  let pair=(distance(&s.key,target),i);
  if pair<best[count-1]{let j=best[..count].partition_point(|&v|v<=pair);best.copy_within(j..count-1,j+1);best[j]=pair;}
 }
 let (d,i)=best[0];let nearest=training[i].lab;
 if d==0{return Ok((nearest,nearest,nearest,0.))}
 let mut idw=[0.;3];let mut total=0.;
 for &(d,i) in &best[..K]{let w=1./d as f64;total+=w;for j in 0..3{idw[j]+=w*training[i].lab[j];}}
 for v in &mut idw{*v/=total;}
 let dim=target.len()+1;
 let unit=SCALE*100.;let unit2=unit*unit;
 let radius=if tapered{best[LOCAL_K].0 as f64}else{1.};
 let weight=|d:u64|{let w=1./(d as f64/unit2+0.01);if tapered{w*(1.-d as f64/radius).clamp(0.,1.).powi(2)}else{w}};
 let wsum=best[..LOCAL_K].iter().map(|&(d,_)|weight(d)).sum::<f64>();
 if !wsum.is_finite()||wsum<=0.{return Err("Degenerate local kernel; no supported weighted neighborhood".into())}
 // Augmented normal equations [X'WX | X'Wy], max 16 predictors.
 let mut a=[[0f64;19];16];
 for &(d,i) in &best[..LOCAL_K]{
  let s=training[i];let w=weight(d)/wsum;
  let mut x=[0.;16];x[0]=1.;
  for j in 1..dim{x[j]=(f64::from(s.key[j-1])-f64::from(target[j-1]))/unit;}
  for j in 0..dim{for k in 0..dim{a[j][k]+=w*x[j]*x[k];}for k in 0..3{a[j][dim+k]+=w*x[j]*s.lab[k];}}
 }
 for j in 1..dim{a[j][j]+=RIDGE;}
 // Partial-pivot Gaussian elimination with finite and singularity checks.
 for j in 0..dim{
  let p=(j..dim).max_by(|&x,&y|a[x][j].abs().total_cmp(&a[y][j].abs())).ok_or("Empty regression")?;
  a.swap(j,p);let pivot=a[j][j];if !pivot.is_finite()||pivot.abs()<1e-12{return Err("Singular local regression".into())}
  for k in j..dim+3{a[j][k]/=pivot;}
  let row=a[j];
  for i in 0..dim{if i!=j{let factor=a[i][j];for k in j..dim+3{a[i][k]-=factor*row[k];}}}
 }
 let mut local=[a[0][dim],a[0][dim+1],a[0][dim+2]];
 if tapered{
  // Compact anchor blend fades to zero at nearest-recipe ties; no abrupt
  // substitution at exact recipe coordinates. Radius at most 2 pp.
  let radius=0.02f64.min((best[1].0 as f64).sqrt()/unit);
  let alpha=(1.-(d as f64).sqrt()/unit/radius).clamp(0.,1.).powi(2);
  for j in 0..3{local[j]=(1.-alpha)*local[j]+alpha*nearest[j];}
 }
 for (v,(lo,hi)) in local.iter_mut().zip([(0.,100.),(-128.,127.),(-128.,127.)]){*v=v.clamp(lo,hi);}
 if local.iter().any(|v|!v.is_finite()){return Err("Invalid regression Lab".into())}
 Ok((nearest,idw,local,(d as f64).sqrt()/unit))
}
#[derive(Default)]
struct Errors{nearest:Vec<f64>,idw:Vec<f64>,local:Vec<f64>,dist:Vec<f64>}
impl Errors{
 fn summary(&self,train:usize)->R<Validation>{Ok(Validation{train,test:self.local.len(),nearest:stats(self.nearest.clone())?,idw:stats(self.idw.clone())?,local:stats(self.local.clone())?,nearest_distance:stats(self.dist.clone())?})}
 fn append(&mut self,e:&Self){self.nearest.extend_from_slice(&e.nearest);self.idw.extend_from_slice(&e.idw);self.local.extend_from_slice(&e.local);self.dist.extend_from_slice(&e.dist);}
}
fn validate_split(training:&[&Sample],test:&[&Sample])->R<Errors>{
 if training.len()<LOCAL_K||test.len()<2{return Err("Local regression requires at least 32 training and 2 holdout recipes".into())}
 let mut e=Errors::default();
 for p in test{let (a,b,c,d)=local_prediction(training,&p.key)?;e.nearest.push(de(p.lab,a));e.idw.push(de(p.lab,b));e.local.push(de(p.lab,c));e.dist.push(d);}
 Ok(e)
}
fn active(p:&Sample)->usize{p.key.iter().filter(|&&x|x>0).count()}
/// All five recipe-group folds, plus an optional fixed stress split
/// (>=5 active inks held out; no training overlap). No hyperparameter tuning.
pub fn cross_validate(chart:&Chart,channels:usize)->R<CrossValidation>{
 let (measurement,samples)=grouped(chart,channels)?;
 let mut pooled=Errors::default();let mut folds=Vec::new();
 for f in 0..5{let (test,train):(Vec<_>,Vec<_>)=samples.iter().partition(|p|fold(&p.key)==f);let e=validate_split(&train,&test)?;folds.push(e.summary(train.len())?);pooled.append(&e);}
 let mut active_counts=vec![0;channels+1];for p in &samples{active_counts[active(p)]+=1;}
 let (test,train):(Vec<_>,Vec<_>)=samples.iter().partition(|p|active(p)>=5);
 let stress=if train.len()>=LOCAL_K&&test.len()>=2{Some(validate_split(&train,&test)?.summary(train.len())?)}else{None};
 let unique=samples.len();
 Ok(CrossValidation{channels,input_rows:chart.patches.len(),unique_recipes:unique,duplicate_rows:chart.patches.len()-unique,measurement,folds,pooled:pooled.summary(0)?,stress,active_counts,peak_tac_percent:samples.iter().map(|p|tac(&p.key)).fold(0.,f64::max)})
}
// Experimental inverse search. All recipes are integer percentage-point
// quanta (0.0001 pp); limits are floored before search, not rounded upward.
const INVERSE_BUDGET:usize=1200;
const NO_SUPPORTED_SEED:&str="No supported feasible seed; adjust limits or obtain more characterization data";
#[derive(Clone,Debug)]
pub struct InverseSettings{
 pub caps_percent:Vec<f64>,pub tac_percent:f64,pub tolerance:f64,pub max_recipe_distance:f64,
}
#[derive(Clone,Debug)]
pub struct InverseReport{
 pub target:[f64;3],pub predicted_lab:[f64;3],pub inks_percent:Vec<f64>,
 pub delta_e:f64,pub initial_delta_e:f64,pub tac_percent:f64,
 pub within_tolerance:bool,pub nearest_recipe_distance:f64,pub exact_recipe:bool,
 pub measurement:String,pub training_recipes:usize,pub evaluations:usize,
 pub settings:InverseSettings,pub sensitivity_max_de:Option<f64>,
 pub sensitivity_supported:usize,pub sensitivity_unsupported:usize,
}
#[derive(Clone,Debug)]
struct Candidate{key:Vec<u32>,lab:[f64;3],error:f64,distance:f64}
fn better(a:&Candidate,b:&Candidate)->bool{
 a.error.total_cmp(&b.error).then_with(||a.distance.total_cmp(&b.distance))
  .then_with(||a.key.iter().sum::<u32>().cmp(&b.key.iter().sum::<u32>()))
  .then_with(||a.key.cmp(&b.key)).is_lt()
}
fn units(v:f64)->u32{(v*SCALE).floor() as u32}
fn project(mut q:Vec<u32>,caps:&[u32],limit:u32)->Vec<u32>{
 for (x,&c) in q.iter_mut().zip(caps){*x=(*x).min(c);}
 let sum=q.iter().sum::<u32>();
 if sum>limit{for x in &mut q{*x=(u64::from(*x)*u64::from(limit)/u64::from(sum)) as u32;}}
 q
}
fn feasible_recipe(q:&[u32],caps:&[u32],limit:u32)->bool{
 q.len()==caps.len()&&q.iter().zip(caps).all(|(&x,&c)|x<=c)&&q.iter().sum::<u32>()<=limit
}
fn trial_score(training:&[&Sample],q:Vec<u32>,target:[f64;3],max_distance:f64)->R<Option<Candidate>>{
 let (_,_,lab,distance)=local_prediction(training,&q)?;
 if distance>max_distance+1e-12{return Ok(None)}
 let error=de(lab,target);
 if !error.is_finite()||error<0.{return Err("Invalid inverse color difference".into())}
 Ok(Some(Candidate{key:q,lab,error,distance}))
}
struct Search<'a>{training:Vec<&'a Sample>,target:[f64;3],max_distance:f64,
 cache:BTreeMap<Vec<u32>,Option<Candidate>>,evaluations:usize}
impl Search<'_>{
 fn score(&mut self,q:Vec<u32>)->R<Option<Candidate>>{
  if let Some(v)=self.cache.get(&q){return Ok(v.clone())}
  if self.evaluations>=INVERSE_BUDGET{return Ok(None)}
  self.evaluations+=1;
  let v=trial_score(&self.training,q.clone(),self.target,self.max_distance)?;
  self.cache.insert(q,v.clone());Ok(v)
 }
}
/// Fit an experimental single recipe to declared measured/reference Lab.
/// No ICC is read or written; this is a model estimate, not measured proof.
/// The distance ceiling is a coverage heuristic, not a confidence interval.
pub fn solve_inverse(chart:&Chart,channels:usize,target:[f64;3],settings:InverseSettings)->R<InverseReport>{
 let (mode,samples)=grouped(chart,channels)?;
 solve_samples(&samples,mode,channels,target,settings)
}
fn solve_samples(samples:&[Sample],measurement:String,n:usize,target:[f64;3],settings:InverseSettings)->R<InverseReport>{
 if samples.len()<LOCAL_K||settings.caps_percent.len()!=n||settings.caps_percent.iter().any(|x|!x.is_finite()||!(0.0..=100.).contains(x))
  ||!settings.tac_percent.is_finite()||!(0.0..=100.*n as f64).contains(&settings.tac_percent)
  ||!settings.tolerance.is_finite()||!(0.1..=20.).contains(&settings.tolerance)
  ||!settings.max_recipe_distance.is_finite()||!(0.01..=1.).contains(&settings.max_recipe_distance)
  ||target.iter().any(|x|!x.is_finite())||!(0.0..=100.).contains(&target[0])||target[1..].iter().any(|x|!(-128.0..=127.).contains(x)){
  return Err("Invalid inverse target, limits or insufficient characterization data".into())
 }
 let caps:Vec<_>=settings.caps_percent.iter().map(|&v|units(v)).collect();let limit=units(settings.tac_percent);
 let mut search=Search{training:samples.iter().collect(),target,max_distance:settings.max_recipe_distance,cache:BTreeMap::new(),evaluations:0};
 // Rank measured recipes independently of the model. Seed from feasible
 // recipes and projected nearest colors, plus the unprinted substrate.
 let mut ranked:Vec<_>=samples.iter().map(|s|(de(s.lab,target),s)).collect();
 if ranked.iter().any(|(d,_)|!d.is_finite()){return Err("Invalid characterization color difference".into())}
 ranked.sort_by(|a,b|a.0.total_cmp(&b.0).then_with(||a.1.key.cmp(&b.1.key)));
 let mut seeds=vec![vec![0;n]];
 seeds.extend(ranked.iter().filter(|(_,s)|feasible_recipe(&s.key,&caps,limit)).take(8).map(|(_,s)|s.key.clone()));
 seeds.extend(ranked.iter().take(8).map(|(_,s)|project(s.key.clone(),&caps,limit)));
 seeds.sort();seeds.dedup();
 let mut starts=Vec::new();for q in seeds{if let Some(c)=search.score(q)?{starts.push(c);}}
 starts.sort_by(|a,b|if better(a,b){std::cmp::Ordering::Less}else if better(b,a){std::cmp::Ordering::Greater}else{std::cmp::Ordering::Equal});
 let mut best=starts.first().cloned().ok_or(NO_SUPPORTED_SEED)?;
 let initial_delta_e=best.error;
 // Bounded deterministic multi-start pattern search. Each sweep evaluates
 // single-axis moves and transfers from the largest other active ink.
 // The feasible set includes a closed channel only at exactly zero.
 for mut current in starts.into_iter().take(3){
  for step in [10.,5.,2.,1.,0.5].map(units){
   for _ in 0..2{
    if search.evaluations>=INVERSE_BUDGET{break}
    let mut next=current.clone();
    let total=current.key.iter().sum::<u32>();
    for j in 0..n{
     if caps[j]==0{continue}
     let mut trial=current.key.clone();trial[j]=trial[j].saturating_sub(step);
     if trial!=current.key{if let Some(c)=search.score(trial)?{if better(&c,&next){next=c;}}}
     let mut trial=current.key.clone();trial[j]+=(caps[j]-trial[j]).min(step).min(limit-total);
     if trial!=current.key{if let Some(c)=search.score(trial)?{if better(&c,&next){next=c;}}}
     let donor=(0..n).filter(|&k|k!=j&&current.key[k]>0).max_by_key(|&k|(current.key[k],std::cmp::Reverse(k)));
     if let Some(k)=donor{
      let mut trial=current.key.clone();let d=step.min(trial[k]).min(caps[j]-trial[j]);trial[k]-=d;trial[j]+=d;
      if d>0{if let Some(c)=search.score(trial)?{if better(&c,&next){next=c;}}}
     }
    }
    if !better(&next,&current){break}current=next;
    if better(&current,&best){best=current.clone();}
   }
  }
 }
 if !feasible_recipe(&best.key,&caps,limit){return Err("Internal inverse limit violation".into())}
 // Diagnostic only: measure model response to feasible +/-0.5 pp changes.
 // Not a proof of continuity or a substitute for printing the recipe.
 let mut sensitivity_max_de=None::<f64>;let mut supported=0;let mut unsupported=0;
 for j in 0..n{for increase in [false,true]{
  let mut q=best.key.clone();let step=units(0.5);
  q[j]=if increase{q[j].saturating_add(step)}else{q[j].saturating_sub(step)};
  if q==best.key||!feasible_recipe(&q,&caps,limit){continue}
  match trial_score(&search.training,q,target,settings.max_recipe_distance)?{
   Some(v)=>{let d=de(best.lab,v.lab);if !d.is_finite(){return Err("Invalid sensitivity sample".into())}
    sensitivity_max_de=Some(sensitivity_max_de.unwrap_or(0.).max(d));supported+=1;},
   None=>unsupported+=1,
  }
 }}
 Ok(InverseReport{target,predicted_lab:best.lab,inks_percent:best.key.iter().map(|&v|f64::from(v)/SCALE).collect(),
  delta_e:best.error,initial_delta_e,tac_percent:tac(&best.key),within_tolerance:best.error<=settings.tolerance,
  nearest_recipe_distance:best.distance,exact_recipe:best.distance==0.,measurement,training_recipes:samples.len(),
  evaluations:search.evaluations,settings,sensitivity_max_de,sensitivity_supported:supported,sensitivity_unsupported:unsupported})
}
#[derive(Clone,Debug)]
pub struct InverseRampAudit{
 pub center:[f64;3],pub settings:InverseSettings,pub points:Vec<InverseReport>,
 pub adjacent_max_channel_pp:Vec<f64>,pub adjacent_total_change_pp:Vec<f64>,
 pub max_channel_jump_pp:f64,pub max_predicted_error:f64,pub within_tolerance_count:usize,
}
/// Five independent solutions on a neutral or chromatic lightness ramp.
/// Report discontinuities without silently smoothing or changing recipes.
pub fn audit_inverse_ramp(chart:&Chart,channels:usize,center:[f64;3],settings:InverseSettings)->R<InverseRampAudit>{
 if !center[0].is_finite()||!(2.0..=98.).contains(&center[0]){return Err("Ramp center L* must be 2–98".into())}
 let (mode,samples)=grouped(chart,channels)?;
 let mut points=Vec::with_capacity(5);
 for delta in [-2.,-1.,0.,1.,2.]{
  let target=[center[0]+delta,center[1],center[2]];
  points.push(solve_samples(&samples,mode.clone(),channels,target,settings.clone())?);
 }
 let mut adjacent_max_channel_pp=Vec::new();let mut adjacent_total_change_pp=Vec::new();
 for pair in points.windows(2){
  let differences=pair[0].inks_percent.iter().zip(&pair[1].inks_percent).map(|(a,b)|(a-b).abs());
  adjacent_max_channel_pp.push(differences.clone().fold(0.,f64::max));adjacent_total_change_pp.push(differences.sum());
 }
 Ok(InverseRampAudit{center,settings,max_channel_jump_pp:adjacent_max_channel_pp.iter().copied().fold(0.,f64::max),
  max_predicted_error:points.iter().map(|p|p.delta_e).fold(0.,f64::max),
  within_tolerance_count:points.iter().filter(|p|p.within_tolerance).count(),points,adjacent_max_channel_pp,adjacent_total_change_pp})
}
/// Experimental joint ramp refinement. Penalizes adjacent ink differences
/// in percentage points, subject to existing limits and per-point error guards.
/// No claim of global 3D LUT continuity.
#[derive(Clone,Debug)]
pub struct RegularizedPoint{pub target:[f64;3],pub predicted_lab:[f64;3],pub inks_percent:Vec<f64>,pub delta_e:f64,pub tac_percent:f64,pub distance:f64}
#[derive(Clone,Debug)]
pub struct RegularizedRamp{pub baseline:InverseRampAudit,pub lambda:f64,pub points:Vec<RegularizedPoint>,pub initial_objective:f64,pub objective:f64,pub max_channel_jump_pp:f64,pub max_predicted_error:f64,pub within_tolerance_count:usize,pub evaluations:usize}
fn roughness(a:&[u32],b:&[u32])->f64{distance(a,b) as f64/(SCALE*SCALE)}
fn local_objective(points:&[Candidate],i:usize,c:&Candidate,lambda:f64)->f64{
 c.error*c.error+lambda*((if i>0{roughness(&c.key,&points[i-1].key)}else{0.})+(if i+1<points.len(){roughness(&c.key,&points[i+1].key)}else{0.}))
}
fn ramp_objective(points:&[Candidate],lambda:f64)->f64{
 points.iter().map(|p|p.error*p.error).sum::<f64>()+lambda*points.windows(2).map(|p|roughness(&p[0].key,&p[1].key)).sum::<f64>()
}
pub fn regularize_inverse_ramp(chart:&Chart,channels:usize,center:[f64;3],settings:InverseSettings,lambda:f64)->R<RegularizedRamp>{
 if !lambda.is_finite()||!(0.0..=1.).contains(&lambda){return Err("Invalid continuity weight".into())}
 let baseline=audit_inverse_ramp(chart,channels,center,settings.clone())?;
 let (_,samples)=grouped(chart,channels)?;let training:Vec<_>=samples.iter().collect();
 let caps:Vec<_>=settings.caps_percent.iter().map(|&x|units(x)).collect();let limit=units(settings.tac_percent);
 let mut points=Vec::new();
 for p in &baseline.points{
  // Round-trip the already quantized baseline recipe, never round a cap upward.
  let q=key(&p.inks_percent,channels)?;
  if !feasible_recipe(&q,&caps,limit){return Err("Baseline recipe violates quantized limits".into())}
  points.push(Candidate{key:q,lab:p.predicted_lab,error:p.delta_e,distance:p.nearest_recipe_distance});
 }
 let guards:Vec<_>=baseline.points.iter().map(|p|p.delta_e.max(settings.tolerance)).collect();
 let initial_objective=ramp_objective(&points,lambda);
 let mut cache:BTreeMap<(usize,Vec<u32>),Option<Candidate>>=BTreeMap::new();
 // Four alternating sweeps. Include neighbor mean, halfway blend and
 // projected coordinate steps. A missed baseline may improve but not worsen.
 if lambda>0.{
  for sweep in 0..4{
   for visit in 0..points.len(){
    let i=if sweep%2==0{visit}else{points.len()-1-visit};
    for step in [5.,2.,1.,0.5].map(units){
     let q=points[i].key.clone();let neighbors=[i.checked_sub(1),if i+1<points.len(){Some(i+1)}else{None}];
     let adjacent:Vec<_>=neighbors.into_iter().flatten().collect();
     let avg:Vec<_>=(0..channels).map(|j|adjacent.iter().map(|&k|points[k].key[j]).sum::<u32>()/adjacent.len() as u32).collect();
     let mut trials=vec![project(avg.clone(),&caps,limit),project(q.iter().zip(&avg).map(|(&a,&b)|(a+b)/2).collect(),&caps,limit)];
     for j in 0..channels{if caps[j]==0{continue}for increase in [false,true]{
      let mut t=q.clone();t[j]=if increase{t[j].saturating_add(step)}else{t[j].saturating_sub(step)};
      trials.push(project(t,&caps,limit));
     }}
     trials.sort();trials.dedup();
     let mut best=points[i].clone();
     for t in trials{
      let cache_key=(i,t.clone());
      let c=if let Some(c)=cache.get(&cache_key){c.clone()}else{
       let c=trial_score(&training,t,baseline.points[i].target,settings.max_recipe_distance)?;
       cache.insert(cache_key,c.clone());c
      };
      if let Some(c)=c{
       // No point initially within tolerance may leave it. A missed point
       // cannot exceed its original error. Always compare actual predictions.
       if c.error>guards[i]+1e-12||(baseline.points[i].within_tolerance&&c.error>settings.tolerance){continue}
       let order=local_objective(&points,i,&c,lambda).total_cmp(&local_objective(&points,i,&best,lambda)).then_with(||c.key.cmp(&best.key));
       if order.is_lt(){best=c;}
      }
     }
     if !feasible_recipe(&best.key,&caps,limit){return Err("Regularized recipe violates limits".into())}
     points[i]=best;
    }
   }
  }
 }
 let objective=ramp_objective(&points,lambda);
 if objective>initial_objective+1e-8{return Err("Continuity objective increased".into())}
 let max_channel_jump_pp=points.windows(2).flat_map(|p|p[0].key.iter().zip(&p[1].key)).map(|(&a,&b)|f64::from(a.abs_diff(b))/SCALE).fold(0.,f64::max);
 let max_predicted_error=points.iter().map(|p|p.error).fold(0.,f64::max);
 let within_tolerance_count=points.iter().filter(|p|p.error<=settings.tolerance).count();
 let points=points.into_iter().zip(&baseline.points).map(|(p,b)|RegularizedPoint{target:b.target,predicted_lab:p.lab,inks_percent:p.key.iter().map(|&v|f64::from(v)/SCALE).collect(),delta_e:p.error,tac_percent:tac(&p.key),distance:p.distance}).collect();
 Ok(RegularizedRamp{baseline,lambda,points,initial_objective,objective,max_channel_jump_pp,max_predicted_error,within_tolerance_count,evaluations:cache.len()})
}
// Experimental local 3x3x3 Lab lattice, not an ICC PCS-wide CLUT.
// Missing nodes are explicit and are never substituted with zero ink.
const LATTICE_GRID:usize=3;
const LATTICE_WORK_LIMIT:u64=3_000_000_000;
#[derive(Clone,Debug)]
pub struct LatticeSettings{pub center:[f64;3],pub half_span:[f64;3],pub inverse:InverseSettings,pub lambda:f64}
#[derive(Clone,Debug)]
pub struct LatticeNode{pub index:[usize;3],pub target:[f64;3],pub baseline:Option<RegularizedPoint>,pub refined:Option<RegularizedPoint>}
#[derive(Clone,Debug)]
pub struct LatticeMetrics{
 pub supported_nodes:usize,pub within_tolerance:usize,pub error:Option<ErrorStats>,
 pub axis_edges:[usize;3],pub axis_max_jump_pp:[Option<f64>;3],pub max_tac_percent:Option<f64>,
 pub complete_cells:usize,pub supported_centers:usize,pub centers_within_tolerance:usize,pub center_error:Option<ErrorStats>,
}
#[derive(Clone,Debug)]
pub struct LatticeAudit{
 pub settings:LatticeSettings,pub nodes:Vec<LatticeNode>,pub baseline:LatticeMetrics,pub refined:LatticeMetrics,
 pub initial_objective:f64,pub objective:f64,pub seed_evaluations:usize,pub refinement_evaluations:usize,
 pub estimated_channel_distance_ops:u64,
}
fn lattice_index(c:[usize;3])->usize{(c[0]*LATTICE_GRID+c[1])*LATTICE_GRID+c[2]}
fn lattice_coord(i:usize)->[usize;3]{[i/9,(i/3)%3,i%3]}
fn lattice_neighbors(i:usize)->Vec<(usize,usize)>{
 let c=lattice_coord(i);let mut out=Vec::new();
 for a in 0..3{for d in [-1i32,1]{let k=c[a] as i32+d;if (0..3).contains(&k){let mut q=c;q[a]=k as usize;out.push((a,lattice_index(q)));}}}
 out
}
fn lattice_target(s:&LatticeSettings,c:[usize;3])->[f64;3]{std::array::from_fn(|i|s.center[i]+(c[i] as f64-1.)*s.half_span[i])}
fn lattice_obj(points:&[Option<Candidate>],lambda:f64)->f64{
 let mut value=0.;
 for (i,p) in points.iter().enumerate(){if let Some(p)=p{value+=p.error*p.error;for (_,j) in lattice_neighbors(i){if j>i{if let Some(q)=&points[j]{value+=lambda*roughness(&p.key,&q.key);}}}}}
 value
}
fn lattice_local(points:&[Option<Candidate>],i:usize,c:&Candidate,lambda:f64)->f64{
 c.error*c.error+lambda*lattice_neighbors(i).into_iter().filter_map(|(_,j)|points[j].as_ref()).map(|p|roughness(&p.key,&c.key)).sum::<f64>()
}
fn optional_stats(v:Vec<f64>)->R<Option<ErrorStats>>{if v.is_empty(){Ok(None)}else{stats(v).map(Some)}}
fn lattice_metrics(points:&[Option<Candidate>],training:&[&Sample],s:&LatticeSettings,caps:&[u32],limit:u32)->R<LatticeMetrics>{
 let mut errors=Vec::new();let mut axis_edges=[0;3];let mut axis_max_jump_pp=[None::<f64>;3];let mut peak=None::<f64>;
 for (i,p) in points.iter().enumerate(){if let Some(p)=p{
  if !feasible_recipe(&p.key,caps,limit){return Err("Lattice node violates limits".into())}
  errors.push(p.error);peak=Some(peak.unwrap_or(0.).max(tac(&p.key)));
  for (axis,j) in lattice_neighbors(i){if j>i{if let Some(q)=&points[j]{
   axis_edges[axis]+=1;let jump=p.key.iter().zip(&q.key).map(|(&a,&b)|f64::from(a.abs_diff(b))/SCALE).fold(0.,f64::max);
   axis_max_jump_pp[axis]=Some(axis_max_jump_pp[axis].unwrap_or(0.).max(jump));
  }}}
 }}
 let mut complete_cells=0;let mut center_errors=Vec::new();
 // At the center of a complete cell trilinear weights are all 1/8.
 // Integer floor keeps the interpolated recipe inside the hard limits.
 for l in 0..2{for a in 0..2{for b in 0..2{
  let corners:Vec<_>=(0..8).filter_map(|v|points[lattice_index([l+(v>>2),a+((v>>1)&1),b+(v&1)])].as_ref()).collect();
  if corners.len()!=8{continue}complete_cells+=1;
  let q:Vec<_>=(0..caps.len()).map(|j|corners.iter().map(|p|p.key[j]).sum::<u32>()/8).collect();
  if !feasible_recipe(&q,caps,limit){return Err("Lattice interpolation violates limits".into())}
  let c=[l,a,b];let target=std::array::from_fn(|i|s.center[i]+(c[i] as f64-0.5)*s.half_span[i]);
  if let Some(p)=trial_score(training,q,target,s.inverse.max_recipe_distance)?{center_errors.push(p.error);}
 }}}
 let within_tolerance=errors.iter().filter(|&&v|v<=s.inverse.tolerance).count();
 let centers_within_tolerance=center_errors.iter().filter(|&&v|v<=s.inverse.tolerance).count();
 Ok(LatticeMetrics{supported_nodes:errors.len(),within_tolerance,error:optional_stats(errors)?,axis_edges,axis_max_jump_pp,max_tac_percent:peak,
  complete_cells,supported_centers:center_errors.len(),centers_within_tolerance,center_error:optional_stats(center_errors)?})
}
fn lattice_point(p:&Candidate,target:[f64;3])->RegularizedPoint{RegularizedPoint{target,predicted_lab:p.lab,inks_percent:p.key.iter().map(|&v|f64::from(v)/SCALE).collect(),delta_e:p.error,tac_percent:tac(&p.key),distance:p.distance}}
pub fn audit_lab_lattice(chart:&Chart,channels:usize,s:LatticeSettings)->R<LatticeAudit>{
 let (mode,samples)=grouped(chart,channels)?;
 if samples.len()<LOCAL_K||!s.lambda.is_finite()||!(0.0..=1.).contains(&s.lambda)||s.half_span.iter().any(|x|!x.is_finite()||!(0.5..=25.).contains(x)){
  return Err("Invalid lattice dimensions, weight or characterization size".into())
 }
 for j in 0..3{let (lo,hi)=if j==0{(0.,100.)}else{(-128.,127.)};if !s.center[j].is_finite()||s.center[j]-s.half_span[j]<lo||s.center[j]+s.half_span[j]>hi{return Err("Lab lattice outside supported PCS range".into())}}
 // Conservative budget: 27 independent searches + sensitivity + four
 // refinement sweeps at four scales, two interpolation audits, seed ranking.
 let estimate=(27*(INVERSE_BUDGET+2*channels+16*(2+2*channels)+1)+16) as u64*samples.len() as u64*channels as u64;
 if estimate>LATTICE_WORK_LIMIT{return Err("Lattice exceeds conservative computation budget; reduce characterization size".into())}
 let training:Vec<_>=samples.iter().collect();
 let mut baseline=Vec::with_capacity(27);let mut seed_evaluations=0;
 for i in 0..27{
  let target=lattice_target(&s,lattice_coord(i));
  match solve_samples(&samples,mode.clone(),channels,target,s.inverse.clone()){
   Ok(r)=>{seed_evaluations+=r.evaluations;baseline.push(Some(Candidate{key:key(&r.inks_percent,channels)?,lab:r.predicted_lab,error:r.delta_e,distance:r.nearest_recipe_distance}));}
   Err(e) if e==NO_SUPPORTED_SEED=>baseline.push(None),
   Err(e)=>return Err(e),
  }
 }
 let caps:Vec<_>=s.inverse.caps_percent.iter().map(|&x|units(x)).collect();let limit=units(s.inverse.tac_percent);
 let before=lattice_metrics(&baseline,&training,&s,&caps,limit)?;
 let mut points=baseline.clone();let initial_objective=lattice_obj(&points,s.lambda);
 let mut cache:BTreeMap<(usize,Vec<u32>),Option<Candidate>>=BTreeMap::new();
 if s.lambda>0.{
  for sweep in 0..4{for visit in 0..27{
   let i=if sweep%2==0{visit}else{26-visit};
   let Some(base)=&baseline[i]else{continue};
   let target=lattice_target(&s,lattice_coord(i));
   for step in [5.,2.,1.,0.5].map(units){
    let Some(current)=&points[i]else{continue};let q=current.key.clone();
    let adjacent:Vec<_>=lattice_neighbors(i).into_iter().filter_map(|(_,j)|points[j].as_ref()).collect();
    if adjacent.is_empty(){continue}
    let avg:Vec<_>=(0..channels).map(|j|adjacent.iter().map(|p|p.key[j]).sum::<u32>()/adjacent.len() as u32).collect();
    let mut trials=vec![project(avg.clone(),&caps,limit),project(q.iter().zip(&avg).map(|(&a,&b)|(a+b)/2).collect(),&caps,limit)];
    for j in 0..channels{if caps[j]==0{continue}for inc in [false,true]{let mut t=q.clone();t[j]=if inc{t[j].saturating_add(step)}else{t[j].saturating_sub(step)};trials.push(project(t,&caps,limit));}}
    trials.sort();trials.dedup();let mut best=current.clone();
    for t in trials{
     let ck=(i,t.clone());let c=if let Some(v)=cache.get(&ck){v.clone()}else{let v=trial_score(&training,t,target,s.inverse.max_recipe_distance)?;cache.insert(ck,v.clone());v};
     if let Some(c)=c{
      if c.error>base.error.max(s.inverse.tolerance)+1e-12||(base.error<=s.inverse.tolerance&&c.error>s.inverse.tolerance){continue}
      if lattice_local(&points,i,&c,s.lambda).total_cmp(&lattice_local(&points,i,&best,s.lambda)).then_with(||c.key.cmp(&best.key)).is_lt(){best=c;}
     }
    }
    points[i]=Some(best);
   }
  }}
 }
 let objective=lattice_obj(&points,s.lambda);if objective>initial_objective+1e-8{return Err("Lattice objective increased".into())}
 let after=lattice_metrics(&points,&training,&s,&caps,limit)?;
 let nodes=(0..27).map(|i|{let index=lattice_coord(i);let target=lattice_target(&s,index);LatticeNode{index,target,baseline:baseline[i].as_ref().map(|p|lattice_point(p,target)),refined:points[i].as_ref().map(|p|lattice_point(p,target))}}).collect();
 Ok(LatticeAudit{settings:s,nodes,baseline:before,refined:after,initial_objective,objective,seed_evaluations,refinement_evaluations:cache.len(),estimated_channel_distance_ops:estimate})
}
/// An audit of sampled jumps near exact reference recipes and held-out
/// errors. Tapered model is a candidate only; the inverse solver still uses
/// the beta.36 hard-neighborhood ridge. Distances are not uncertainty bounds.
#[derive(Clone,Debug)]
pub struct ModelPair{pub count:usize,pub hard:ErrorStats,pub tapered:ErrorStats}
#[derive(Clone,Debug)]
pub struct Perturbation{pub step_percent:f64,pub samples:ModelPair}
#[derive(Clone,Debug)]
pub struct ContinuityAudit{pub channels:usize,pub anchors:usize,pub perturbations:Vec<Perturbation>,pub pooled:ModelPair,pub stress:Option<ModelPair>}
fn model_pair(hard:Vec<f64>,tapered:Vec<f64>)->R<ModelPair>{
 if hard.len()!=tapered.len(){return Err("Mismatched audit samples".into())}
 Ok(ModelPair{count:hard.len(),hard:stats(hard)?,tapered:stats(tapered)?})
}
fn audit_holdout(train:&[&Sample],test:&[&Sample],hard:&mut Vec<f64>,tapered:&mut Vec<f64>)->R<()>{
 if train.len()<LOCAL_K+1||test.len()<2{return Err("Continuity audit needs 33 training and 2 holdout recipes".into())}
 for p in test{
  let (_,_,a,_)=local_kernel(train,&p.key,false)?;
  let (_,_,b,_)=local_kernel(train,&p.key,true)?;
  hard.push(de(p.lab,a));tapered.push(de(p.lab,b));
 }Ok(())
}
pub fn audit_continuity(chart:&Chart,channels:usize)->R<ContinuityAudit>{
 let (_,samples)=grouped(chart,channels)?;
 if samples.len()<LOCAL_K+1{return Err("Insufficient unique recipes for continuity audit".into())}
 let training:Vec<_>=samples.iter().collect();
 let anchors=samples.len().min(128);
 let mut perturbations=Vec::new();
 for step in [0.01,0.1,0.5]{
  let mut hard=Vec::new();let mut tapered=Vec::new();
  for i in 0..anchors{
   let p=&samples[i*(samples.len()-1)/(anchors-1)];
   for j in 0..channels{for sign in [-1i64,1]{
    let mut q=p.key.clone();let v=i64::from(q[j])+sign*i64::from(units(step));
    if !(0..=i64::from(units(100.))).contains(&v){continue}
    q[j]=v as u32;
    let (_,_,a,_)=local_kernel(&training,&q,false)?;
    let (_,_,b,_)=local_kernel(&training,&q,true)?;
    hard.push(de(p.lab,a));tapered.push(de(p.lab,b));
   }}
  }
  perturbations.push(Perturbation{step_percent:step,samples:model_pair(hard,tapered)?});
 }
 let mut hard=Vec::new();let mut tapered=Vec::new();
 for f in 0..5{let (test,train):(Vec<_>,Vec<_>)=samples.iter().partition(|p|fold(&p.key)==f);audit_holdout(&train,&test,&mut hard,&mut tapered)?;}
 let pooled=model_pair(hard,tapered)?;
 let (test,train):(Vec<_>,Vec<_>)=samples.iter().partition(|p|active(p)>=5);
 let stress=if train.len()>=LOCAL_K+1&&test.len()>=2{
  let mut a=Vec::new();let mut b=Vec::new();audit_holdout(&train,&test,&mut a,&mut b)?;Some(model_pair(a,b)?)
 }else{None};
 Ok(ContinuityAudit{channels,anchors,perturbations,pooled,stress})
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
  let cv=cross_validate(&c,3).unwrap();assert_eq!(cv.pooled.test,126);assert_eq!(cv.folds.iter().map(|f|f.test).sum::<usize>(),126);assert!(cv.pooled.local.mean<cv.pooled.idw.mean);assert!(cv.stress.is_none());assert_eq!(cv.active_counts.iter().sum::<usize>(),126);
  c.patches[0].measurement="M0".into();assert!(evaluate(&c,3,0).is_err());
 }
 #[test]fn inverse_limits_seed_and_determinism(){
  let samples=(0..=10).flat_map(|i|(0..=10).map(move|j|Sample{key:vec![units(i as f64*10.),units(j as f64*10.),0],lab:[90.-i as f64*4.,j as f64*5.,0.]})).collect::<Vec<_>>();
  let settings=InverseSettings{caps_percent:vec![65.,60.,0.],tac_percent:90.,tolerance:2.,max_recipe_distance:0.4};
  let a=solve_samples(&samples,"M1".into(),3,[70.,20.,0.],settings.clone()).unwrap();
  let b=solve_samples(&samples,"M1".into(),3,[70.,20.,0.],settings.clone()).unwrap();
  assert_eq!(a.inks_percent,b.inks_percent);assert_eq!(a.inks_percent[2],0.);
  assert!(a.tac_percent<=90.&&a.inks_percent[0]<=65.&&a.inks_percent[1]<=60.);
  assert!(a.delta_e<=a.initial_delta_e&&a.within_tolerance&&a.evaluations<=INVERSE_BUDGET);
  let z=solve_samples(&samples,"M1".into(),3,[20.,80.,80.],InverseSettings{caps_percent:vec![0.;3],tac_percent:0.,..settings.clone()}).unwrap();
  assert_eq!(z.inks_percent,vec![0.;3]);assert!(!z.within_tolerance);assert_eq!(z.evaluations,1);
  assert!(solve_samples(&samples,"M1".into(),3,[50.,0.,0.],InverseSettings{tac_percent:f64::NAN,..settings}).is_err());
 }
 #[test]fn regularized_ramp_preserves_limits_and_tolerance(){
  let mut text=String::from("CTI3\nILLUMINANT D50\nOBSERVER 2\nMEASUREMENT_CONDITION M1\nBEGIN_DATA_FORMAT\nSAMPLE_ID CH1 CH2 CH3 LAB_L LAB_A LAB_B\nEND_DATA_FORMAT\nBEGIN_DATA\n");
  for i in 0..=10{for j in 0..=10{text.push_str(&format!("p-{i}-{j} {} {} 0 {} {} 0\n",i*10,j*10,90-i*4,j*5));}}
  text.push_str("END_DATA\n");let chart=crate::characterization_cgats::read(&text,3,false).unwrap();
  let settings=InverseSettings{caps_percent:vec![65.,60.,0.],tac_percent:90.,tolerance:2.,max_recipe_distance:0.4};
  let r=regularize_inverse_ramp(&chart,3,[70.,20.,0.],settings.clone(),0.02).unwrap();
  assert!(r.objective<=r.initial_objective+1e-8);
  for (p,b) in r.points.iter().zip(&r.baseline.points){assert_eq!(p.inks_percent[2],0.);assert!(p.tac_percent<=90.);assert!(p.delta_e<=b.delta_e.max(2.)+1e-12);if b.within_tolerance{assert!(p.delta_e<=2.);}}
  let z=regularize_inverse_ramp(&chart,3,[70.,20.,0.],settings.clone(),0.).unwrap();assert_eq!(z.evaluations,0);assert_eq!(z.objective,z.initial_objective);assert!((z.max_channel_jump_pp-z.baseline.max_channel_jump_pp).abs()<1e-9);
  assert!(regularize_inverse_ramp(&chart,3,[70.,20.,0.],settings,f64::NAN).is_err());
 }
 #[test]fn lattice_topology_and_missing_metrics(){
  let edges=(0..27).map(|i|lattice_neighbors(i).into_iter().filter(|&(_,j)|j>i).count()).sum::<usize>();assert_eq!(edges,54);
  for i in 0..27{assert_eq!(lattice_index(lattice_coord(i)),i);}
  let settings=LatticeSettings{center:[50.,0.,0.],half_span:[1.;3],inverse:InverseSettings{caps_percent:vec![0.;2],tac_percent:0.,tolerance:2.,max_recipe_distance:0.1},lambda:0.02};
  let m=lattice_metrics(&vec![None;27],&[],&settings,&[0;2],0).unwrap();assert_eq!(m.supported_nodes,0);assert_eq!(m.complete_cells,0);assert!(m.error.is_none());assert_eq!(m.axis_max_jump_pp,[None;3]);
 }
 #[test]fn lattice_refinement_keeps_bounds_and_audits_centers(){
  let mut t=String::from("CTI3\nILLUMINANT D50\nOBSERVER 2\nMEASUREMENT_CONDITION M1\nBEGIN_DATA_FORMAT\nSAMPLE_ID CH1 CH2 CH3 LAB_L LAB_A LAB_B\nEND_DATA_FORMAT\nBEGIN_DATA\n");
  for i in 0..=5{for j in 0..=5{for k in 0..=5{t.push_str(&format!("p-{i}-{j}-{k} {} {} {} {} {} {}\n",i*20,j*20,k*20,90-i*10,j*10-20,k*10-20));}}}
  t.push_str("END_DATA\n");let chart=crate::characterization_cgats::read(&t,3,false).unwrap();
  let settings=LatticeSettings{center:[70.,0.,0.],half_span:[1.;3],inverse:InverseSettings{caps_percent:vec![100.;3],tac_percent:150.,tolerance:2.,max_recipe_distance:0.4},lambda:0.02};
  let a=audit_lab_lattice(&chart,3,settings.clone()).unwrap();assert_eq!(a.nodes.len(),27);assert_eq!(a.refined.supported_nodes,27);assert_eq!(a.refined.axis_edges,[18;3]);assert_eq!(a.refined.complete_cells,8);assert_eq!(a.refined.supported_centers,8);assert!(a.objective<=a.initial_objective+1e-8);
  for n in &a.nodes{let p=n.refined.as_ref().unwrap();assert!(p.tac_percent<=150.0000001);let b=n.baseline.as_ref().unwrap();if b.delta_e<=2.{assert!(p.delta_e<=2.);}}
  let z=audit_lab_lattice(&chart,3,LatticeSettings{lambda:0.,..settings}).unwrap();assert_eq!(z.refinement_evaluations,0);assert_eq!(z.objective,z.initial_objective);
 }
 #[test]fn inverse_quantized_limits_and_unsupported_zero(){
  let caps=[units(10.00009),units(20.00009)];let limit=units(19.99999);
  let q=project(vec![1000000;2],&caps,limit);assert!(feasible_recipe(&q,&caps,limit));
  let samples=(0..32).map(|i|Sample{key:vec![900000+i*100,900000],lab:[50.,0.,0.]}).collect::<Vec<_>>();
  let s=InverseSettings{caps_percent:vec![0.;2],tac_percent:0.,tolerance:2.,max_recipe_distance:0.1};
  assert!(solve_samples(&samples,"M1".into(),2,[50.,0.,0.],s).is_err());
 }
 #[test]fn tapered_model_preserves_anchor_and_reduces_small_jump(){
  let samples=(0..=10).flat_map(|i|(0..=10).map(move|j|Sample{key:vec![units(i as f64*10.),units(j as f64*10.)],lab:[90.-i as f64*4.,j as f64*5.+if i==5&&j==5{4.}else{0.},0.]})).collect::<Vec<_>>();
  let train:Vec<_>=samples.iter().collect();let p=&samples[60];
  assert_eq!(local_kernel(&train,&p.key,true).unwrap().2,p.lab);
  let mut q=p.key.clone();q[0]+=units(0.01);
  let a=de(p.lab,local_kernel(&train,&q,false).unwrap().2);
  let b=de(p.lab,local_kernel(&train,&q,true).unwrap().2);
  assert!(b<a&&b<0.1,"{a} / {b}");
  assert!(local_kernel(&train[..32],&q,true).is_err());
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
  let ramp=audit_inverse_ramp(&c,7,[50.,0.,0.],InverseSettings{caps_percent:vec![100.,100.,100.,0.,100.,100.,100.],tac_percent:300.,tolerance:2.,max_recipe_distance:0.4}).unwrap();
  println!("{ramp:#?}");assert_eq!(ramp.points.len(),5);
  for p in &ramp.points{assert_eq!(p.inks_percent[3],0.);assert!(p.tac_percent<=300.0000001);assert!(p.delta_e<=p.initial_delta_e);}
  assert_eq!(ramp.adjacent_max_channel_pp.len(),4);assert_eq!(ramp.within_tolerance_count,5);
  let lattice=audit_lab_lattice(&c,7,LatticeSettings{center:[50.,0.,0.],half_span:[1.;3],inverse:ramp.settings.clone(),lambda:0.02}).unwrap();
  assert_eq!(lattice.refined.supported_nodes,27);assert_eq!(lattice.refined.within_tolerance,27);assert_eq!(lattice.refined.complete_cells,8);assert_eq!(lattice.refined.supported_centers,8);assert_eq!(lattice.refined.centers_within_tolerance,8);
  println!("lattice baseline {:?} / refined {:?}",lattice.baseline,lattice.refined);
  assert!(lattice.objective<lattice.initial_objective);assert!(lattice.refined.axis_max_jump_pp.iter().all(|v|v.is_some_and(|x|x<3.)));
  assert!((lattice.refined.center_error.as_ref().unwrap().mean-0.2071886).abs()<0.05);
  let smooth=regularize_inverse_ramp(&c,7,[50.,0.,0.],ramp.settings.clone(),0.02).unwrap();println!("{smooth:#?}");
  assert_eq!(smooth.within_tolerance_count,5);assert!(smooth.max_channel_jump_pp<=2.);assert!(smooth.objective<smooth.initial_objective);
  for p in &smooth.points{assert_eq!(p.inks_percent[3],0.);assert!(p.tac_percent<=300.0000001);}

  let audit=audit_continuity(&c,7).unwrap();println!("{audit:#?}");
  assert_eq!(audit.anchors,128);assert_eq!(audit.perturbations[0].samples.count,1237);
  assert!((audit.perturbations[0].samples.hard.max-1.35925).abs()<0.01);
  assert!((audit.perturbations[0].samples.tapered.max-0.01081).abs()<0.01);
  assert!((audit.pooled.tapered.mean-1.08499).abs()<0.01);
  assert!((audit.stress.as_ref().unwrap().tapered.p95-9.38617).abs()<0.01);
  let cv=cross_validate(&c,7).unwrap();println!("{cv:#?}");assert_eq!(cv.pooled.test,4272);assert_eq!(cv.stress.as_ref().unwrap().test,413);assert_eq!(cv.active_counts,vec![1,119,308,1406,2025,339,73,1]);assert!((cv.pooled.local.mean-1.2086).abs()<0.01);assert!((cv.stress.unwrap().local.mean-3.4681).abs()<0.01);
 }
}
