//! Constrained printer profile rebuilding for nCLR output profiles.
//! Invert the original media-relative native forward pipeline. All three
//! reverse intents share the regenerated separation (as in Custom CMYK).
//! A2B and device channel identity stay intact; not a new characterization.
use crate::{Profile,Lut,R,print_space,editing};
use lcms2::{Tag,TagSignature};

pub fn channel_count(p:&Profile)->R<usize>{
 if &p.header[12..16]!=b"prtr"||p.cmyk()||p.pcs()!="Lab "||!matches!(p.version_major(),2|4){return Err("Requires nCLR output ICC v2/v4 with Lab PCS".into())}
 if p.tags.keys().any(|k|k.starts_with(b"D2B")||k.starts_with(b"B2D")){return Err("Alternate D2B/B2D pipelines are not supported".into())}
 let n=print_space::format(&p.header[16..20])?.channels;
 let a=p.tags.get(b"A2B1").ok_or("Missing A2B1")?;
 if !(p.version_major()==4&&a.starts_with(b"mAB ")||p.version_major()==2&&(a.starts_with(b"mft1")||a.starts_with(b"mft2"))){return Err("Unsupported forward LUT encoding".into())}
 Ok(n)
}
fn settings(n:usize,caps:&[f64],tac:f64,grid:usize)->R<()>{
 if caps.len()!=n||caps.iter().any(|x|!x.is_finite()||!(0.0..=1.).contains(x))||!tac.is_finite()||!(0.0..=n as f64).contains(&tac)||![9,17,25,33].contains(&grid){return Err("Invalid multichannel limits or grid".into())}Ok(())
}
fn feasible(x:&[f64],caps:&[f64],tac:f64)->bool{
 x.iter().zip(caps).all(|(&v,&c)|v.is_finite()&&v>=0.&&v<=c)&&x.iter().sum::<f64>()<=tac+1e-12
}
fn constrain(x:&mut [f64],caps:&[f64],tac:f64){
 for (v,&c) in x.iter_mut().zip(caps){*v=v.clamp(0.,c);}
 let total=x.iter().sum::<f64>();if total>tac&&total>0.{for v in x{*v*=tac/total;}}
}
#[derive(Debug,Clone)]
pub struct Verified{pub channels:Vec<f64>,pub tac:f64,pub sampled_channels:Vec<f64>,pub sampled_tac:f64,pub samples:usize}
pub fn verify(p:&Profile,caps:&[f64],tac:f64)->R<Verified>{
 let n=if p.cmyk(){crate::editing::editable_channel_count(p)?}else{channel_count(p)?};settings(n,caps,tac,9)?;
 let b=print_space::bounds(p)?;
 if !b.exact_node_tac||b.channels.iter().zip(caps).any(|(&v,&c)|v>c+1e-10)||b.tac>tac+1e-10{return Err("Cannot certify rebuilt channel/TAC bounds".into())}
 let native=lcms2::Profile::new_icc(&p.bytes()?).map_err(|e|e.to_string())?;
 let mut out=Verified{channels:b.channels,tac:b.tac,sampled_channels:vec![0.;n],sampled_tac:0.,samples:0};
 for tag in [TagSignature::BToA0Tag,TagSignature::BToA1Tag,TagSignature::BToA2Tag]{
  let Tag::Pipeline(pipe)=native.read_tag(tag)else{return Err("Missing native reverse pipeline".into())};
  if pipe.input_channels()!=3||pipe.output_channels() as usize!=n{return Err("Unexpected native pipeline dimensions".into())}
  let mut values=vec![0f32;n];
  for l in 0..=16{for a in 0..=16{for b in 0..=16{
   pipe.eval(&[l as f32/16.,a as f32/16.,b as f32/16.],&mut values);
   for j in 0..n{
    let v=f64::from(values[j]);
    if !v.is_finite()||v< -0.00002||v>caps[j]+2./65535.||(caps[j]==0.&&v!=0.){return Err(format!("Native channel {} violates limit",j+1))}
    out.sampled_channels[j]=out.sampled_channels[j].max(v);
   }
   let t=values.iter().map(|&x|f64::from(x)).sum::<f64>();
   if t>tac+n as f64*2./65535.{return Err("Native sampled TAC exceeds limit".into())}
   out.sampled_tac=out.sampled_tac.max(t);out.samples+=1;
  }}}
 }
 Ok(out)
}
pub fn rebuild(p:&Profile,caps:&[f64],tac:f64,grid:usize,progress:impl Fn(String))->R<Profile>{
 let n=channel_count(p)?;settings(n,caps,tac,grid)?;
 let native=lcms2::Profile::new_icc(&p.bytes()?).map_err(|e|e.to_string())?;
 let Tag::Pipeline(forward)=native.read_tag(TagSignature::AToB1Tag)else{return Err("Cannot decode native A2B1".into())};
 if forward.input_channels() as usize!=n||forward.output_channels()!=3{return Err("Unexpected native forward dimensions".into())}
 let Tag::Pipeline(reverse)=native.read_tag(TagSignature::BToA1Tag)else{return Err("Requires native B2A1 for initial separation seed".into())};
 if reverse.input_channels()!=3||reverse.output_channels() as usize!=n{return Err("Unexpected native reverse dimensions".into())}
 let scale=if p.version_major()==2{[100.*65535./65280.,255.*65535./65280.,255.*65535./65280.]}else{[100.,255.,255.]};
 let eval=|x:&[f64]|->R<[f64;3]>{
  let mut input=[0f32;15];for(j,&v)in x.iter().enumerate(){input[j]=v as f32;}
  let mut y=[0f32;3];forward.eval(&input[..n],&mut y);
  if y.iter().any(|v|!v.is_finite()||*v< -0.00002||*v>1.00002){return Err("Invalid forward PCS result".into())}
  Ok(y.map(|v|f64::from(v).clamp(0.,1.)))
 };
 let dist=|a:[f64;3],b:[f64;3]|(0..3).map(|j|((a[j]-b[j])*scale[j]).powi(2)).sum::<f64>();
 // Bounded seed pool: zero, per-ink ramps, and deterministic low-discrepancy
 // mixtures. No exponential N-dimensional Cartesian lattice allocation.
 let mut seeds=Vec::new();
 let zero=vec![0.;n];seeds.push((zero.clone(),eval(&zero)?));
 for j in 0..n{for f in [0.25,0.5,0.75,1.]{
  let mut x=zero.clone();x[j]=caps[j]*f;constrain(&mut x,caps,tac);seeds.push((x.clone(),eval(&x)?));
 }}
 let primes=[2usize,3,5,7,11,13,17,19,23,29,31,37,41,43,47];
 fn radical(mut k:usize,b:usize)->f64{let(mut f,mut v)=(1.,0.);while k>0{f/=b as f64;v+=f*(k%b) as f64;k/=b;}v}
 for i in 1..=256{
  let mut x:Vec<_>=(0..n).map(|j|caps[j]*radical(i,primes[j])).collect();constrain(&mut x,caps,tac);seeds.push((x.clone(),eval(&x)?));
 }
 let mut table=Vec::with_capacity(grid.pow(3)*n);
 let mut seed=[0f32;15];let den=(grid-1) as f64;
 for l in 0..grid{
  progress(format!("Multichannel inversion: slice {}/{} · {} channels",l+1,grid,n));
  for a in 0..grid{for b in 0..grid{
   let target=[l as f64/den,a as f64/den,b as f64/den];
   reverse.eval(&target.map(|v|v as f32),&mut seed[..n]);
   if seed[..n].iter().any(|v|!v.is_finite()){return Err("Invalid source separation".into())}
   let mut best:Vec<_>=seed[..n].iter().map(|&v|f64::from(v)).collect();constrain(&mut best,caps,tac);
   let mut cost=dist(eval(&best)?,target);
   for (x,lab) in &seeds{let d=dist(*lab,target);if d+1e-12<cost{best.clone_from(x);cost=d;}}
   let mut trial=best.clone();
   let mut step=0.25;
   for _ in 0..9{
    for _ in 0..2{
     let previous=cost;
     for j in 0..n{for dir in [-1.,1.]{
      trial.clone_from(&best);trial[j]=(trial[j]+dir*step).clamp(0.,caps[j]);
      if feasible(&trial,caps,tac){let d=dist(eval(&trial)?,target);if d+1e-12<cost{best.clone_from(&trial);cost=d;}}
     }}
     // Pair transfers permit motion on a saturated TAC boundary.
     for j in 0..n{for k in j+1..n{for dir in [-1.,1.]{
      trial.clone_from(&best);
      let (from,to)=if dir>0.{(j,k)}else{(k,j)};
      let delta=step.min(trial[from]).min(caps[to]-trial[to]);
      if delta<=0.{continue}
      trial[from]-=delta;trial[to]+=delta;
      if feasible(&trial,caps,tac){let d=dist(eval(&trial)?,target);if d+1e-12<cost{best.clone_from(&trial);cost=d;}}
     }}}
     if previous-cost<=1e-12{break}
    }
    step*=0.5;
   }
   // Quantize downward so the identity output curves cannot increase caps.
   for v in best{table.push((v.clamp(0.,1.)*65535.).floor()/65535.);}
  }}
 }
 let lut=Lut{inputs:3,outputs:n,grid,in_curves:vec![vec![0.,1.];3],out_curves:vec![vec![0.,1.];n],clut:table};
 let tag=editing::encode(&lut,if p.version_major()==4{b"mBA "}else{b"mft2"})?;
 let mut result=p.clone();
 for k in [b"B2A0",b"B2A1",b"B2A2"]{result.tags.insert(*k,tag.clone());}
 result.clean_stale();verify(&result,caps,tac)?;Ok(result)
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn six_channel_rebuild_and_zero_channel(){
  let p=print_space::fixture(6);let caps=[0.8,0.3,0.4,0.7,0.,0.25];
  let out=rebuild(&p,&caps,1.2,9,|_|{}).unwrap();
  for k in [b"A2B0",b"A2B1",b"A2B2"]{assert_eq!(p.tags[k],out.tags[k]);}
  let v=verify(&out,&caps,1.2).unwrap();assert_eq!(v.channels[4],0.);assert_eq!(v.sampled_channels[4],0.);assert_eq!(v.samples,14739);assert!(v.tac<=1.2+1e-10);assert_eq!(out.channels().len(),6);
 }
 #[test]fn reject_wrong_caps_and_alternate(){
  let mut p=print_space::fixture(6);assert!(rebuild(&p,&[1.;4],1.,9,|_|{}).is_err());assert!(rebuild(&p,&[1.;6],7.,9,|_|{}).is_err());p.tags.insert(*b"B2D0",vec![0;8]);assert!(channel_count(&p).is_err());
 }
 #[test]fn all_closed(){
  let p=print_space::fixture(3);let caps=[0.;3];let out=rebuild(&p,&caps,0.,9,|_|{}).unwrap();
  let v=verify(&out,&caps,0.).unwrap();assert_eq!(v.tac,0.);assert_eq!(v.sampled_tac,0.);
 }
}
