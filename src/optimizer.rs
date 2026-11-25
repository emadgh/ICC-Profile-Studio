//! Predictive, bounded ink recipe search. The model is the ICC's native
//! relative-colorimetric forward transform; costs are user-provided relative
//! weights, not measured ink mass or monetary costs.
use lcms2::{CIELab,CIELabExt,CIEXYZ,CIEXYZExt,Intent,PixelFormat,Profile,Transform};
use std::path::Path;
type R<T>=Result<T,String>;
#[derive(Clone,Copy,Debug)]
pub struct Settings{pub caps:[f64;4],pub tac:f64,pub weights:[f64;4],pub tolerance:f64}
impl Default for Settings{fn default()->Self{Self{caps:[100.;4],tac:240.,weights:[1.;4],tolerance:2.}}}
impl Settings{
 pub fn validate(self)->R<()>{
  if self.caps.iter().any(|v|!v.is_finite()||!(0.0..=100.).contains(v))||!self.tac.is_finite()||!(0.0..=400.).contains(&self.tac)||self.weights.iter().any(|v|!v.is_finite()||!(0.01..=100.).contains(v))||!self.tolerance.is_finite()||!(0.0..=20.).contains(&self.tolerance){return Err("Invalid recipe constraints".into())}Ok(())
 }
}
#[derive(Clone,Debug)]
pub struct Recipe{pub ink:[f64;4],pub lab:[f64;3],pub delta_e:f64,pub cost:f64}
#[derive(Clone,Debug)]
pub struct Report{pub baseline:Recipe,pub closest:Recipe,pub cheapest:Option<Recipe>,pub evaluated:usize}
fn raw(v:[f64;4])->[u16;4]{v.map(|v|(v.clamp(0.,100.)/100.*65535.).floor() as u16)}
fn pct(v:[u16;4])->[f64;4]{v.map(|v|v as f64*100./65535.)}
fn allowed(v:[f64;4],s:Settings)->bool{v.iter().zip(s.caps).all(|(&x,c)|x.is_finite()&&x>=0.&&x<=c+1e-9)&&v.iter().sum::<f64>()<=s.tac+1e-9}
fn lab(v:[f64;3])->CIELab{CIEXYZ{X:v[0],Y:v[1],Z:v[2]}.to_lab(&CIEXYZ{X:0.9642,Y:1.,Z:0.8249})}
fn recipe(ink:[u16;4],xyz:[f64;3],target:CIELab,s:Settings)->R<Recipe>{
 let l=lab(xyz);let de=l.cie2000_delta_e(&target,1.,1.,1.);if !de.is_finite()||de<0.{return Err("Invalid predicted Lab".into())}
 let ink=pct(ink);let cost=ink.iter().zip(s.weights).map(|(v,w)|v*w).sum();
 Ok(Recipe{ink,lab:[l.L,l.a,l.b],delta_e:de,cost})
}
fn rank(r:Recipe,closest:&mut Option<Recipe>,cheap:&mut Option<Recipe>,tolerance:f64){
 if closest.as_ref().is_none_or(|c|r.delta_e<c.delta_e||(r.delta_e==c.delta_e&&r.cost<c.cost)){*closest=Some(r.clone())}
 if r.delta_e<=tolerance&&cheap.as_ref().is_none_or(|c|r.cost<c.cost||(r.cost==c.cost&&r.delta_e<c.delta_e)){*cheap=Some(r)}
}
pub fn find(path:&Path,target:[f64;3],s:Settings)->R<Report>{
 s.validate()?;if target.iter().any(|x|!x.is_finite())||!(0.0..=100.).contains(&target[0])||target[1..].iter().any(|x|!(-128.0..=127.).contains(x)){return Err("Invalid D50 Lab target".into())}
 let printer=crate::comparison::open_printer(path)?;let xyz=Profile::new_xyz();let target=CIELab{L:target[0],a:target[1],b:target[2]};
 let white=CIEXYZ{X:0.9642,Y:1.,Z:0.8249};let x=target.to_xyz(&white);
 let forward:Transform<[u16;4],[f64;3]>=Transform::new(&printer,PixelFormat::CMYK_16,&xyz,PixelFormat::XYZ_DBL,Intent::RelativeColorimetric).map_err(|e|e.to_string())?;
 let reverse:Transform<[f64;3],[u16;4]>=Transform::new(&xyz,PixelFormat::XYZ_DBL,&printer,PixelFormat::CMYK_16,Intent::RelativeColorimetric).map_err(|e|e.to_string())?;
 let mut base=[[0u16;4];1];reverse.transform_pixels(&[[x.X,x.Y,x.Z]],&mut base);
 let mut bx=[[0f64;3];1];forward.transform_pixels(&base,&mut bx);let baseline=recipe(base[0],bx[0],target,s)?;
 let mut candidates=Vec::new();
 for i in 0..9usize.pow(4){let mut q=i;let mut v=[0.;4];for j in (0..4).rev(){v[j]=s.caps[j]*(q%9) as f64/8.;q/=9;}if allowed(v,s){candidates.push(raw(v))}}
 if allowed(baseline.ink,s){candidates.push(base[0])}
 let mut closest=None;let mut cheap=None;let mut evaluated=0;
 let mut eval=|candidates:&[[u16;4]],closest:&mut Option<Recipe>,cheap:&mut Option<Recipe>|->R<()>{
  let mut values=vec![[0f64;3];candidates.len()];forward.transform_pixels(candidates,&mut values);
  for (&v,x) in candidates.iter().zip(values){rank(recipe(v,x,target,s)?,closest,cheap,s.tolerance);}evaluated+=candidates.len();Ok(())
 };
 eval(&candidates,&mut closest,&mut cheap)?;
 for step in [8.,4.,2.,1.,0.5,0.25]{
  for _ in 0..4{
   let mut next=Vec::new();
   for seed in [closest.as_ref(),cheap.as_ref()].into_iter().flatten(){
    for i in 0..4{for sign in [-1.,1.]{let mut v=seed.ink;v[i]+=sign*step;if allowed(v,s){next.push(raw(v));}}}
    for i in 0..4{for j in 0..4{if i==j{continue}let mut v=seed.ink;v[i]-=step;v[j]+=step;if allowed(v,s){next.push(raw(v));}}}
   }
   if !next.is_empty(){eval(&next,&mut closest,&mut cheap)?;}
  }
 }
 Ok(Report{baseline,closest:closest.ok_or("No feasible recipes")?,cheapest:cheap,evaluated})
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn caps_and_tac(){let s=Settings{caps:[100.,100.,100.,0.],tac:200.,..Default::default()};assert!(allowed([80.,80.,40.,0.],s));assert!(!allowed([80.,80.,41.,0.],s));assert!(!allowed([0.,0.,0.,1.],s));}
 #[test]fn ranking(){let mut a=None;let mut b=None;for (de,cost) in [(1.,8.),(1.5,5.),(3.,1.)]{rank(Recipe{ink:[0.;4],lab:[0.;3],delta_e:de,cost},&mut a,&mut b,2.);}assert_eq!(a.unwrap().cost,8.);assert_eq!(b.unwrap().cost,5.);}
}

// Dynamic recipes use bounded candidate batches instead of an exponential
// Cartesian lattice. All device values are quantized before evaluation.
#[derive(Clone,Debug)]
pub struct MultiSettings{pub caps:Vec<f64>,pub tac:f64,pub weights:Vec<f64>,pub tolerance:f64}
impl MultiSettings{
 pub fn validate(&self,n:usize)->R<()>{
  if !(2..=15).contains(&n)||self.caps.len()!=n||self.weights.len()!=n||self.caps.iter().any(|v|!v.is_finite()||!(0.0..=100.).contains(v))||!self.tac.is_finite()||!(0.0..=n as f64*100.).contains(&self.tac)||self.weights.iter().any(|v|!v.is_finite()||!(0.01..=100.).contains(v))||!self.tolerance.is_finite()||!(0.1..=5.).contains(&self.tolerance){return Err("Invalid multichannel recipe constraints".into())}
  Ok(())
 }
}
#[derive(Clone,Debug)]
pub struct MultiRecipe{pub ink:Vec<f64>,pub lab:[f64;3],pub delta_e:f64,pub cost:f64,pub tac:f64,pub feasible:bool}
#[derive(Clone,Debug)]
pub struct MultiReport{pub baseline:MultiRecipe,pub closest:MultiRecipe,pub cheapest:Option<MultiRecipe>,pub evaluated:usize,pub target:[f64;3],pub settings:MultiSettings}
fn encode_recipe(v:&[f64])->Vec<u8>{v.iter().flat_map(|x|((x.clamp(0.,100.)*65535./100.).floor() as u16).to_ne_bytes()).collect()}
fn decode_recipe(v:&[u8])->Vec<f64>{v.chunks_exact(2).map(|x|u16::from_ne_bytes([x[0],x[1]]) as f64*100./65535.).collect()}
fn feasible(v:&[f64],s:&MultiSettings)->bool{v.len()==s.caps.len()&&v.iter().zip(&s.caps).all(|(&x,&c)|x.is_finite()&&x>=0.&&x<=c+1e-9)&&v.iter().sum::<f64>()<=s.tac+1e-9}
fn project(v:&mut [f64],s:&MultiSettings){for (x,&c) in v.iter_mut().zip(&s.caps){*x=x.clamp(0.,c);}let sum=v.iter().sum::<f64>();if sum>s.tac&&sum>0.{for x in v{*x*=s.tac/sum;}}}
fn multi_recipe(ink:Vec<f64>,xyz:[f64;3],target:CIELab,s:&MultiSettings)->R<MultiRecipe>{
 let l=lab(xyz);let delta_e=l.cie2000_delta_e(&target,1.,1.,1.);
 if !delta_e.is_finite()||delta_e<0.||![l.L,l.a,l.b].iter().all(|x|x.is_finite()){return Err("Invalid predicted Lab".into())}
 let cost=ink.iter().zip(&s.weights).map(|(v,w)|v*w).sum();let tac=ink.iter().sum();
 let feasible=feasible(&ink,s);
 Ok(MultiRecipe{ink,lab:[l.L,l.a,l.b],delta_e,cost,tac,feasible})
}
fn rank_multi(r:MultiRecipe,closest:&mut Option<MultiRecipe>,cheap:&mut Option<MultiRecipe>,tol:f64){
 if !r.feasible{return}
 if closest.as_ref().is_none_or(|c|r.delta_e<c.delta_e||(r.delta_e==c.delta_e&&r.cost<c.cost)){*closest=Some(r.clone());}
 if r.delta_e<=tol&&cheap.as_ref().is_none_or(|c|r.cost<c.cost||(r.cost==c.cost&&r.delta_e<c.delta_e)){*cheap=Some(r);}
}
pub fn find_multi(path:&Path,target:[f64;3],mut s:MultiSettings)->R<MultiReport>{
 use lcms2::Flags;
 let parsed=crate::Profile::read(path)?;
 let printer=Profile::new_icc(&parsed.bytes()?).map_err(|e|e.to_string())?;
 let fmt=crate::print_space::for_profile(&printer)?;let n=fmt.channels;s.validate(n)?;
 // Preserve closed source channels, even when the user sets a larger cap.
 // Fail closed on unsupported strict LUT structures rather than guess caps.
 let bounds=crate::print_space::bounds(&parsed)?;
 for (j,v) in s.caps.iter_mut().enumerate(){*v=v.min(bounds.channels[j]*100.);}
 if target.iter().any(|x|!x.is_finite())||!(0.0..=100.).contains(&target[0])||target[1..].iter().any(|x|!(-128.0..=127.).contains(x)){return Err("Invalid D50 Lab target".into())}
 let xyz=Profile::new_xyz();let target_lab=CIELab{L:target[0],a:target[1],b:target[2]};
 let white=CIEXYZ{X:0.9642,Y:1.,Z:0.8249};let x=target_lab.to_xyz(&white);
 let forward:Transform<u8,[f64;3]>=Transform::new_multiprofile(&[&printer,&xyz],fmt.pixel,PixelFormat::XYZ_DBL,Intent::RelativeColorimetric,Flags::NO_OPTIMIZE).map_err(|e|e.to_string())?;
 let reverse:Transform<[f64;3],u8>=Transform::new_multiprofile(&[&xyz,&printer],PixelFormat::XYZ_DBL,fmt.pixel,Intent::RelativeColorimetric,Flags::NO_OPTIMIZE).map_err(|e|e.to_string())?;
 let mut base=vec![0u8;2*n];reverse.transform_pixels(&[[x.X,x.Y,x.Z]],&mut base);
 let mut bx=[[0.;3];1];forward.transform_pixels(&base,&mut bx);
 let baseline=multi_recipe(decode_recipe(&base),bx[0],target_lab,&s)?;
 let mut closest=None;let mut cheap=None;let mut evaluated=0usize;
 let mut seen=std::collections::HashSet::<Vec<u8>>::new();
 let mut eval=|rows:Vec<Vec<f64>>,closest:&mut Option<MultiRecipe>,cheap:&mut Option<MultiRecipe>|->R<()>{
  let mut packed=Vec::new();for v in rows{
   if !feasible(&v,&s){continue}
   let raw=encode_recipe(&v);if seen.len()>=40_000{break}
   if seen.insert(raw.clone()){packed.extend(raw);}
  }
  let count=packed.len()/(2*n);if count==0{return Ok(())}
  let mut values=vec![[0.;3];count];forward.transform_pixels(&packed,&mut values);
  for (row,x) in packed.chunks_exact(2*n).zip(values){rank_multi(multi_recipe(decode_recipe(row),x,target_lab,&s)?,closest,cheap,s.tolerance);}
  evaluated+=count;Ok(())
 };
 // Include the exact native baseline in ranking, without requantization.
 rank_multi(baseline.clone(),&mut closest,&mut cheap,s.tolerance);
 let mut seed=baseline.ink.clone();project(&mut seed,&s);
 let mut candidates=vec![vec![0.;n],seed];
 // Deterministic distributed points, plus low-dimensional ramps.
 let mut state=0x41c6_4e6du32;
 for _ in 0..4096{
  let mut v=s.caps.iter().map(|&c|{state^=state<<13;state^=state>>17;state^=state<<5;c*(state as f64/u32::MAX as f64)}).collect::<Vec<_>>();
  project(&mut v,&s);candidates.push(v);
 }
 for j in 0..n{for i in 0..=16{let mut v=vec![0.;n];v[j]=s.caps[j]*i as f64/16.;project(&mut v,&s);candidates.push(v);}}
 eval(candidates,&mut closest,&mut cheap)?;
 for step in [10.,5.,2.,1.,0.5,0.25]{
  for _ in 0..3{
   let mut next=Vec::new();
   for seed in [closest.as_ref(),cheap.as_ref()].into_iter().flatten(){
    for i in 0..n{
     if s.caps[i]==0.{continue}
     for sign in [-1.,1.]{let mut v=seed.ink.clone();v[i]+=sign*step;if feasible(&v,&s){next.push(v);}}
     for j in 0..n{if i!=j&&s.caps[j]>0.{let mut v=seed.ink.clone();v[i]-=step;v[j]+=step;if feasible(&v,&s){next.push(v);}}}
    }
   }
   eval(next,&mut closest,&mut cheap)?;
  }
 }
 Ok(MultiReport{baseline,closest:closest.ok_or("No feasible recipes")?,cheapest:cheap,evaluated:evaluated+1,target,settings:s})
}
#[cfg(test)]
mod multi_tests{
 use super::*;
 #[test]fn dynamic_recipe_caps_and_target(){
  let d=tempfile::tempdir().unwrap();let p=d.path().join("six.icc");crate::print_space::fixture(6).save_new(&p).unwrap();
  let s=MultiSettings{caps:vec![100.;6],tac:200.,weights:vec![1.,2.,1.,1.,1.,1.],tolerance:2.};
  let r=find_multi(&p,[50.,0.,0.],s).unwrap();assert_eq!(r.closest.ink.len(),6);assert_eq!(r.closest.ink[5],0.);assert!(r.closest.tac<=200.+1e-9);assert!(r.evaluated<=40_001);
  if let Some(c)=r.cheapest{assert!(c.feasible&&c.delta_e<=2.);assert_eq!(c.ink[5],0.);}
 }
 #[test]fn invalid_settings(){let s=MultiSettings{caps:vec![100.;6],tac:601.,weights:vec![1.;6],tolerance:2.};assert!(s.validate(6).is_err());assert!(s.validate(4).is_err());}
}
