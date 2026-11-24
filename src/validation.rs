//! Conservative mathematical bounds for supported LUTs, plus independent
//! LittleCMS checks. Sample results are never called an exhaustive proof.
use super::{Profile,Lut,R,interp};
use lcms2::{Tag,TagSignature};
#[derive(Debug,Clone)]
pub struct Bounds{pub channels:[f64;4],pub tac:f64,pub exact_node_tac:bool}
pub(crate) fn interval_peak(curve:&[f64],lo:f64,hi:f64)->f64{
 let mut peak=interp(curve,lo).max(interp(curve,hi));
 let scale=(curve.len()-1) as f64;
 let start=(lo*scale).ceil() as usize;let end=((hi*scale).floor() as usize).min(curve.len()-1);
 if start<=end{for &v in &curve[start..=end]{peak=peak.max(v);}}
 peak
}
fn lut_bound(l:&Lut)->R<(f64,bool)>{
 if (l.inputs,l.outputs)!=(3,4){return Err("Expected 3→4 output LUT".into())}
 // Two samples define an affine curve over the entire normalized interval.
 // The sum of affine post-curves is affine in the interpolated CLUT output.
 // The maximum is therefore attained at a node.
 if l.out_curves.iter().all(|c|c.len()==2){
  let peak=l.clut.chunks_exact(4).map(|n|(0..4).map(|j|interp(&l.out_curves[j],n[j])).sum::<f64>()).fold(0.,f64::max);
  return Ok((peak,true))
 }
 // Otherwise, bound each cell using the CLUT component extrema and all
 // sampled post-curve extrema within those intervals. The sum is an upper
 // bound, not necessarily attainable at one point.
 let g=l.grid;let mut peak=0f64;
 for x in 0..g-1{for y in 0..g-1{for z in 0..g-1{
  let mut lo=[1f64;4];let mut hi=[0f64;4];
  for dx in 0..2{for dy in 0..2{for dz in 0..2{
   let idx=(((x+dx)*g+y+dy)*g+z+dz)*4;
   for j in 0..4{let v=l.clut[idx+j];lo[j]=lo[j].min(v);hi[j]=hi[j].max(v);}
  }}}
  let sum=(0..4).map(|j|interval_peak(&l.out_curves[j],lo[j],hi[j])).sum::<f64>();
  peak=peak.max(sum);
 }}}
 Ok((peak,false))
}
pub fn bounds(p:&Profile)->R<Bounds>{
 let channels=p.verify([1.;4])?;
 let mut tac=0f64;let mut exact=true;
 for key in [b"B2A0",b"B2A1",b"B2A2"]{
  let lut=Lut::parse(p.tags.get(key).ok_or("Missing B2A intent")?)?;
  let (peak,is_exact)=lut_bound(&lut)?;tac=tac.max(peak);exact &=is_exact;
 }
 Ok(Bounds{channels,tac,exact_node_tac:exact})
}
#[derive(Debug,Clone)]
pub struct NativeSamples{pub count:usize,pub channels:[f64;4],pub tac:f64}
pub fn native_samples(p:&Profile)->R<NativeSamples>{
 let profile=lcms2::Profile::new_icc(&p.bytes()?).map_err(|e|format!("LittleCMS could not open ICC: {e}"))?;
 let mut r=NativeSamples{count:0,channels:[0.;4],tac:0.};
 for tag in [TagSignature::BToA0Tag,TagSignature::BToA1Tag,TagSignature::BToA2Tag]{
  let Tag::Pipeline(t)=profile.read_tag(tag)else{return Err("LittleCMS could not read a B2A pipeline".into())};
  if t.input_channels()!=3||t.output_channels()!=4{return Err("Unexpected native B2A dimensions".into())}
  for x in 0..=16{for y in 0..=16{for z in 0..=16{
   let mut v=[0f32;4];t.eval(&[x as f32/16.,y as f32/16.,z as f32/16.],&mut v);
   if v.iter().any(|v|!v.is_finite()||*v< -0.00002||*v>1.00002){return Err("Invalid native B2A device output".into())}
   let v=v.map(|v|v as f64);for j in 0..4{r.channels[j]=r.channels[j].max(v[j]);}
   r.tac=r.tac.max(v.iter().sum::<f64>());r.count+=1;
  }}}
 }
 Ok(r)
}
pub fn verify_output(p:&Profile,caps:[f64;4],tac:Option<f64>)->R<(Bounds,NativeSamples)>{
 let b=bounds(p)?;
 if b.channels.iter().zip(caps).any(|(&v,c)|v>c+1e-10){return Err("Strict channel limit exceeded".into())}
 if let Some(t)=tac{if b.tac>t+1e-10{return Err(format!("Cannot certify requested TAC {:.3}%: {} bound {:.3}%. Use the Custom engine or a different limit.",100.*t,if b.exact_node_tac{"node"}else{"conservative cell"},100.*b.tac))}}
 let n=native_samples(p)?;
 let eps=2./65535.;
 if n.channels.iter().zip(caps).any(|(&v,c)|v>c+eps)||tac.is_some_and(|t|n.tac>t+4.*eps){return Err("Independent LittleCMS samples exceed requested ink limits (including rounding tolerance)".into())}
 Ok((b,n))
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn native_zero_k_and_bound(){
  let p=crate::synthetic_cmyk_v4(true);
  let (b,n)=verify_output(&p,[1.,1.,1.,0.],Some(1.)).unwrap();
  assert!(b.exact_node_tac);assert_eq!(b.channels[3],0.);assert_eq!(n.channels[3],0.);assert_eq!(n.count,14739);
  assert!(verify_output(&p,[1.,1.,1.,0.],Some(0.1)).is_err());
 }
 #[test]fn nonlinear_curve_peak_between_nodes(){
  let l=Lut{inputs:3,outputs:4,grid:2,in_curves:vec![vec![0.,1.];3],clut:(0..8).flat_map(|i|[if i%2==0{0.}else{1.},0.,0.,0.]).collect(),out_curves:vec![vec![0.,1.,0.],vec![0.,1.],vec![0.,1.],vec![0.,1.]]};
  let (t,exact)=lut_bound(&l).unwrap();assert_eq!(t,1.);assert!(!exact);
 }
}
