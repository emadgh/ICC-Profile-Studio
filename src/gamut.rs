//! Sample the effective separation gamut via LittleCMS, not only the
//! unchanged physical A2B model. Both profiles use the same D50 Lab targets.
use lcms2::{Profile,Intent,CIELab,CIELabExt,CIEXYZ,CIEXYZExt};
use std::path::Path;
#[derive(Clone,Debug)]
pub struct Cloud {pub points:Vec<[f32;3]>,pub sampled:usize,pub errors:Vec<f64>,pub surface:Option<crate::gamut_surface::Surface>}
#[derive(Clone,Debug)]
pub struct Overlay {pub reference:Result<Cloud,String>,pub candidate:Option<Result<Cloud,String>>}
fn targets(grid:usize)->Vec<[f64;3]>{
 let mut out=Vec::with_capacity(grid.pow(3));let n=(grid-1) as f64;
 for l in 0..grid{for a in 0..grid{for b in 0..grid{
  out.push([100.*l as f64/n,-128.+255.*a as f64/n,-128.+255.*b as f64/n]);
 }}}
 out
}
pub fn sample(path:&Path,grid:usize)->Result<Cloud,String>{sample_with_intent(path,grid,Intent::RelativeColorimetric)}
/// The requested intent applies to both separation and appearance legs.
/// Native LittleCMS may use standard fallback for missing intent tables.
pub fn sample_with_intent(path:&Path,grid:usize,intent:Intent)->Result<Cloud,String>{
 crate::analysis_job::check()?;
 if !(5..=33).contains(&grid){return Err("Grid must be 5..33".into())}
 if std::fs::metadata(path).map_err(|e|e.to_string())?.len()>128*1024*1024{return Err("ICC exceeds 128 MiB".into())}
 let p=Profile::new_icc(&std::fs::read(path).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
 let native=crate::print_space::Roundtrip::new(&p,intent)?;
 let white=CIEXYZ{X:0.9642,Y:1.,Z:0.8249};
 let values:Vec<[f64;3]>=targets(grid).into_iter().filter_map(|v|{
  let t=CIELab{L:v[0],a:v[1],b:v[2]}.to_xyz(&white);
  let xyz=[t.X,t.Y,t.Z];
  xyz.iter().all(|v|v.is_finite()&&*v>=0.&&*v<2.).then_some(xyz)
 }).collect();
 if values.is_empty(){return Err("No valid PCS targets".into())}
 crate::analysis_job::check()?;let result=native.apply(&values)?;
 let mut points=Vec::with_capacity(result.len());let mut errors=Vec::with_capacity(result.len());
 for (v,target) in result.into_iter().zip(&values){
  let l=CIEXYZ{X:v[0],Y:v[1],Z:v[2]}.to_lab(&white);
  if ![l.L,l.a,l.b].iter().all(|v|v.is_finite()){return Err("Non-finite Lab result".into())}
  let target=CIEXYZ{X:target[0],Y:target[1],Z:target[2]}.to_lab(&white);
  let error=l.cie2000_delta_e(&target,1.,1.,1.);if !error.is_finite()||error<0.{return Err("Invalid Lab target error".into())}errors.push(error);
  points.push([l.L as f32,l.a as f32,l.b as f32]);
 }
 // Duplicate projected colors are common when targets lie outside the gamut.
 // Quantize only the display point keys, retain the original Lab coordinates.
 let mut seen=std::collections::HashSet::new();
 points.retain(|p|seen.insert(p.map(|v|(v*4.).round() as i32)));
 if points.is_empty(){return Err("LittleCMS produced no usable gamut points".into())}
 // Adaptive Lab box concentrates samples on the actual printer gamut.
 // Padding allows tolerance contours to extend beyond sampled extrema.
 let mut lo=[f32::INFINITY;3];let mut hi=[f32::NEG_INFINITY;3];
 for v in &points{for j in 0..3{lo[j]=lo[j].min(v[j]);hi[j]=hi[j].max(v[j]);}}
 for j in 0..3{lo[j]-=12.;hi[j]+=12.;}
 lo[0]=lo[0].max(-5.);hi[0]=hi[0].min(105.);
 let n=if grid<=9{25usize}else if grid<=17{41}else{57};
 let mut field=vec![20.;n.pow(3)];let mut sample_xyz=Vec::new();let mut sample_indices=Vec::new();let mut target_lab=Vec::new();
 for x in 1..n-1{crate::analysis_job::check()?;for y in 1..n-1{for z in 1..n-1{
  let v:[f32;3]=std::array::from_fn(|j|lo[j]+(hi[j]-lo[j])*[x,y,z][j] as f32/(n-1) as f32);
  if !(0. ..=100.).contains(&v[0]){continue}
  let l=CIELab{L:v[0] as f64,a:v[1] as f64,b:v[2] as f64};let t=l.to_xyz(&white);
  let xyz=[t.X,t.Y,t.Z];if xyz.iter().all(|v|v.is_finite()&&*v>=0.&&*v<2.){
   sample_xyz.push(xyz);sample_indices.push((x*n+y)*n+z);target_lab.push(l);
  }
 }}}
 if !sample_xyz.is_empty(){
  let actual=native.apply(&sample_xyz)?;
  for ((i,lab),v) in sample_indices.into_iter().zip(target_lab).zip(actual){
   let l=CIEXYZ{X:v[0],Y:v[1],Z:v[2]}.to_lab(&white);
   let e=l.cie2000_delta_e(&lab,1.,1.,1.);if e.is_finite()&&e>=0.{field[i]=e.min(20.);}
  }
 }
 crate::analysis_job::check()?;let surface=crate::gamut_surface::extract(&field,n,lo,hi,crate::gamut_surface::TOLERANCE);
 crate::analysis_job::check()?;Ok(Cloud{points,sampled:values.len(),errors,surface})
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct Coverage{pub total:usize,pub both:usize,pub lost:usize,pub gained:usize,pub neither:usize}
pub fn coverage(a:&Cloud,b:&Cloud,threshold:f64)->Option<Coverage>{
 if a.errors.len()!=b.errors.len()||a.errors.is_empty()||!threshold.is_finite()||threshold<0.{return None}
 let mut r=Coverage{total:a.errors.len(),both:0,lost:0,gained:0,neither:0};
 for (&x,&y) in a.errors.iter().zip(&b.errors){match (x<=threshold,y<=threshold){(true,true)=>r.both+=1,(true,false)=>r.lost+=1,(false,true)=>r.gained+=1,(false,false)=>r.neither+=1}}
 Some(r)
}
pub fn overlay(reference:&Path,candidate:Option<&Path>,grid:usize)->Overlay{
 overlay_with_intents(reference,candidate,grid,Intent::RelativeColorimetric,Intent::RelativeColorimetric)
}
pub fn overlay_with_intents(reference:&Path,candidate:Option<&Path>,grid:usize,reference_intent:Intent,candidate_intent:Intent)->Overlay{
 let _=crate::analysis_job::phase("Sampling profile gamut",if candidate.is_some(){2}else{1});
 let reference=sample_with_intent(reference,grid,reference_intent);let _=crate::analysis_job::advance();
 let candidate=candidate.map(|p|{let result=sample_with_intent(p,grid,candidate_intent);let _=crate::analysis_job::advance();result});
 Overlay{reference,candidate}
}
/// Equal Lab scaling, orthographic projection. x=a*, y=b*, z=L*-50.
pub fn project(lab:[f32;3],yaw:f32,pitch:f32)->[f32;3]{
 let [l,a,b]=lab;let z=l-50.;
 let x=a*yaw.cos()-b*yaw.sin();let y=a*yaw.sin()+b*yaw.cos();
 [x,z*pitch.cos()-y*pitch.sin(),z*pitch.sin()+y*pitch.cos()]
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn coverage_partitions(){let a=Cloud{points:vec![],sampled:4,errors:vec![1.,1.,3.,3.],surface:None};let b=Cloud{errors:vec![1.,3.,1.,3.],..a.clone()};assert_eq!(coverage(&a,&b,2.).unwrap(),Coverage{total:4,both:1,lost:1,gained:1,neither:1});}
 #[test]fn grid_endpoints(){let t=targets(9);assert_eq!(t.len(),729);assert_eq!(t[0],[0.,-128.,-128.]);assert_eq!(t[728],[100.,127.,127.]);}
 #[test]fn projection_origin(){assert_eq!(project([50.,0.,0.],0.7,0.5),[0.,0.,0.]);}
 #[test]fn rotation_preserves_length(){let q=project([80.,30.,-20.],1.2,0.8);assert!((q.iter().map(|v|v*v).sum::<f32>()-2200.).abs()<0.01);}
 #[test]fn six_channel_gamut(){
  let d=tempfile::tempdir().unwrap();let p=d.path().join("six.icc");crate::print_space::fixture(6).save_new(&p).unwrap();
  let r=sample(&p,9).unwrap();assert!(r.sampled>0);assert!(!r.points.is_empty());assert!(r.errors.iter().all(|e|e.is_finite()));
 }
 #[test]fn requested_intents_use_same_target_grid(){
  let d=tempfile::tempdir().unwrap();let p=d.path().join("six.icc");crate::print_space::fixture(6).save_new(&p).unwrap();
  let relative=sample(&p,5).unwrap();
  for intent in [Intent::Perceptual,Intent::RelativeColorimetric,Intent::Saturation,Intent::AbsoluteColorimetric]{
   let result=overlay_with_intents(&p,Some(&p),5,Intent::RelativeColorimetric,intent);
   let a=result.reference.unwrap();let b=result.candidate.unwrap().unwrap();
   assert_eq!(a.errors,relative.errors);assert_eq!(a.sampled,b.sampled);
   assert_eq!(b.errors,sample_with_intent(&p,5,intent).unwrap().errors);
   assert!(coverage(&a,&b,2.).is_some());
  }
 }
 #[test]fn rejects_rgb(){let d=tempfile::tempdir().unwrap();let p=d.path().join("rgb.icc");std::fs::write(&p,Profile::new_srgb().icc().unwrap()).unwrap();assert!(sample(&p,9).is_err());}
}
