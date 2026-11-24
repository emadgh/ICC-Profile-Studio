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
