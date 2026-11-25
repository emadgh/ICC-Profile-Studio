//! ICC curveType and parametricCurveType, normalized device domain.
//! Analytic extrema include both sides of each piecewise breakpoint.
//! Unsupported/non-finite/out-of-domain definitions fail closed.
use crate::{R,u16at,u32at,interp};
#[derive(Clone,Debug)]
pub(crate) enum Curve{Sampled(Vec<f64>),Power(f64),Parametric{kind:u16,p:Vec<f64>}}
#[derive(Clone,Copy,Debug)]
pub(crate) struct RangeInfo{pub min:f64,pub max:f64,pub monotonic:bool}
impl Curve{
 pub(crate) fn read(t:&[u8])->R<Self>{
  if t.len()<12{return Err("Truncated curve".into())}
  let c=match &t[..4]{
   b"curv"=>{let n=u32at(t,8)? as usize;
    if n>65536||12+2*n>t.len(){return Err("Invalid sampled curve length".into())}
    if n==0{Self::Sampled(vec![0.,1.])}
    else if n==1{Self::Power(u16at(t,12)? as f64/256.)}
    else{Self::Sampled(t[12..12+2*n].chunks_exact(2).map(|v|u16::from_be_bytes([v[0],v[1]]) as f64/65535.).collect())}
   },
   b"para"=>{let kind=u16at(t,8)?;let n=* [1usize,3,4,5,7].get(kind as usize).ok_or("Unsupported parametric function")?;
    let mut p=Vec::with_capacity(n);for i in 0..n{p.push(u32at(t,12+4*i)? as i32 as f64/65536.);}
    if kind==0{Self::Power(p[0])}else{Self::Parametric{kind,p}}
   },
   _=>return Err("Unknown curve encoding".into())
  };
  c.range(0.,1.)?;Ok(c)
 }
 fn breakpoint(kind:u16,p:&[f64])->R<f64>{
  if p[0]<=0.||p[1]<=0.{return Err("Nonpositive parametric exponent or scale".into())}
  Ok(if kind<=2{-p[2]/p[1]}else{p[4]})
 }
 fn branch(kind:u16,p:&[f64],x:f64,upper:bool)->R<f64>{
  let y=if upper{
   let base=p[1]*x+p[2];
   if base< -1e-12{return Err("Negative power base".into())}
   base.max(0.).powf(p[0])+match kind{2=>p[3],4=>p[5],_=>0.}
  }else{match kind{1=>0.,2=>p[3],3=>p[3]*x,4=>p[3]*x+p[6],_=>return Err("Invalid parametric type".into())}};
  if !y.is_finite()||y< -1e-12||y>1.+1e-12{return Err("Analytic curve exceeds normalized device range".into())}
  Ok(y.clamp(0.,1.))
 }
 pub(crate) fn eval(&self,x:f64)->R<f64>{
  if !x.is_finite()||!(0.0..=1.).contains(&x){return Err("Curve input outside normalized domain".into())}
  match self{
   Self::Sampled(v)=>Ok(interp(v,x)),
   Self::Power(g)=>{if !g.is_finite()||*g<=0.{Err("Nonpositive gamma".into())}else{Ok(x.powf(*g))}},
   Self::Parametric{kind,p}=>Self::branch(*kind,p,x,x>=Self::breakpoint(*kind,p)?)
  }
 }
 pub(crate) fn affine(&self)->bool{matches!(self,Self::Sampled(v) if v.len()==2)||matches!(self,Self::Power(g) if *g==1.)}
 pub(crate) fn range(&self,lo:f64,hi:f64)->R<RangeInfo>{
  if !lo.is_finite()||!hi.is_finite()||lo<0.||hi>1.||lo>hi{return Err("Invalid curve interval".into())}
  let mut min=self.eval(lo)?;let mut max=min;let mut last=min;let mut monotonic=true;
  let mut push=|y:f64|{min=min.min(y);max=max.max(y);if y<last{monotonic=false;}last=y;};
  match self{
   Self::Sampled(v)=>{let scale=(v.len()-1) as f64;
    let start=(lo*scale).ceil() as usize;let end=(hi*scale).floor() as usize;
    if start<=end{for &y in &v[start..=end]{push(y);}}
   },
   Self::Power(_)=>{},
   Self::Parametric{kind,p}=>{
    let d=Self::breakpoint(*kind,p)?;
    if lo<d&&d<=hi{
     // Include the left-hand limit and actual right branch; jumps are
     // represented in both extrema and monotonicity.
     push(Self::branch(*kind,p,d,false)?);
     push(Self::branch(*kind,p,d,true)?);
    }
   }
  }
  push(self.eval(hi)?);
  Ok(RangeInfo{min,max,monotonic})
 }
 /// Explicit approximation after gamma composition. Bound the error in
 /// every interval by the full analytic range and the two quantized knots.
 /// A steep curve or a discontinuity may be refused instead of flattened.
 pub(crate) fn gamma_table(&self,g:f64,count:usize,tolerance:f64)->R<(Vec<u16>,f64)>{
  if !g.is_finite()||!(0.25..=4.).contains(&g)||!(2..=65535).contains(&count)||!tolerance.is_finite()||tolerance<0.{return Err("Invalid gamma approximation settings".into())}
  if !self.range(0.,1.)?.monotonic{return Err("Gamma editing requires a nondecreasing output curve".into())}
  let mut table=Vec::with_capacity(count);
  for i in 0..count{table.push((self.eval(i as f64/(count-1) as f64)?.powf(g)*65535.).floor() as u16);}
  let mut error=0f64;
  for i in 0..count-1{
   let r=self.range(i as f64/(count-1) as f64,(i+1) as f64/(count-1) as f64)?;
   let a=table[i] as f64/65535.;let b=table[i+1] as f64/65535.;
   error=error.max((r.max.powf(g)-a).abs()).max((b-r.min.powf(g)).abs());
   if error>tolerance{return Err(format!("Curve approximation exceeds {:.3} percentage points; original curve retained",100.*tolerance))}
  }
  Ok((table,error))
 }
}
#[cfg(test)]
mod tests{
 use super::*;
 fn para(kind:u16,p:&[f64])->Curve{Curve::Parametric{kind,p:p.to_vec()}}
 #[test]fn analytic_families_and_signed_parameters(){
  let g=Curve::Power(2.);assert!((g.eval(0.5).unwrap()-0.25).abs()<1e-12);
  let c=para(1,&[2.,1.,-0.25]);assert_eq!(c.eval(0.1).unwrap(),0.);assert!((c.eval(0.75).unwrap()-0.25).abs()<1e-12);
  let c=para(2,&[1.,0.5,-0.1,0.1]);assert!((c.eval(0.).unwrap()-0.1).abs()<1e-12);
  let c=para(3,&[1.,0.5,0.,0.25,0.5]);let r=c.range(0.,1.).unwrap();assert!(r.monotonic);assert_eq!(r.max,0.5);
  let c=para(4,&[1.,0.5,0.,0.5,0.5,0.,0.]);assert_eq!(c.eval(0.25).unwrap(),0.125);
 }
 #[test]fn downward_jump_is_included_in_bounds(){
  let c=para(4,&[1.,0.5,0.,1.,0.5,0.,0.]);let r=c.range(0.,0.5).unwrap();
  assert!(!r.monotonic);assert_eq!(r.max,0.5);assert_eq!(c.eval(0.5).unwrap(),0.25);
  assert!(c.gamma_table(1.,4096,0.005).is_err());
 }
 #[test]fn invalid_or_outside_normalized_range_refused(){
  assert!(Curve::Power(0.).range(0.,1.).is_err());
  assert!(para(3,&[0.5,1.,-0.8,0.1,0.5]).range(0.,1.).is_err());
  assert!(para(2,&[1.,1.,0.,0.5]).range(0.,1.).is_err());
 }
 #[test]fn approximation_bound_and_zero(){
  let c=Curve::Power(2.);let (v,e)=c.gamma_table(1.5,16385,0.005).unwrap();
  assert_eq!(v[0],0);assert_eq!(*v.last().unwrap(),65535);assert!(e<=0.005);
  for i in 0..1000{let x=i as f64/999.;let pos=x*(v.len()-1) as f64;let j=pos.floor() as usize;let k=(j+1).min(v.len()-1);let y=(v[j] as f64+(v[k] as f64-v[j] as f64)*(pos-j as f64))/65535.;assert!((y-x.powf(3.)).abs()<=e+1e-12);}
 }
}
