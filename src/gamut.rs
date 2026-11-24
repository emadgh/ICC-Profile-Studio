//! Sample the effective separation gamut via LittleCMS, not only the
//! unchanged physical A2B model. Both profiles use the same D50 Lab targets.
use lcms2::{Profile,PixelFormat,Transform,Intent,CIELab,CIELabExt,CIEXYZ,CIEXYZExt,ColorSpaceSignature,ProfileClassSignature};
use std::path::Path;
#[derive(Clone,Debug)]
pub struct Cloud {pub points:Vec<[f32;3]>,pub sampled:usize}
#[derive(Clone,Debug)]
pub struct Overlay {pub reference:Result<Cloud,String>,pub candidate:Option<Result<Cloud,String>>}
fn targets(grid:usize)->Vec<[f64;3]>{
 let mut out=Vec::with_capacity(grid.pow(3));let n=(grid-1) as f64;
 for l in 0..grid{for a in 0..grid{for b in 0..grid{
  out.push([100.*l as f64/n,-128.+255.*a as f64/n,-128.+255.*b as f64/n]);
 }}}
 out
}
pub fn sample(path:&Path,grid:usize)->Result<Cloud,String>{
 if !(5..=33).contains(&grid){return Err("Grid must be 5..33".into())}
 if std::fs::metadata(path).map_err(|e|e.to_string())?.len()>128*1024*1024{return Err("ICC exceeds 128 MiB".into())}
 let p=Profile::new_icc(&std::fs::read(path).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
 if p.color_space()!=ColorSpaceSignature::CmykData||p.device_class()!=ProfileClassSignature::Output{return Err("Gamut preview requires a CMYK printer ICC.".into())}
 let xyz=Profile::new_xyz();let white=CIEXYZ{X:0.9642,Y:1.,Z:0.8249};
 let values:Vec<[f64;3]>=targets(grid).into_iter().filter_map(|v|{
  let t=CIELab{L:v[0],a:v[1],b:v[2]}.to_xyz(&white);
  let xyz=[t.X,t.Y,t.Z];
  xyz.iter().all(|v|v.is_finite()&&*v>=0.&&*v<2.).then_some(xyz)
 }).collect();
 if values.is_empty(){return Err("No valid PCS targets".into())}
 let separation:Transform<[f64;3],[u16;4]>=Transform::new(&xyz,PixelFormat::XYZ_DBL,&p,PixelFormat::CMYK_16,Intent::RelativeColorimetric).map_err(|e|e.to_string())?;
 let appearance:Transform<[u16;4],[f64;3]>=Transform::new(&p,PixelFormat::CMYK_16,&xyz,PixelFormat::XYZ_DBL,Intent::RelativeColorimetric).map_err(|e|e.to_string())?;
 let mut device=vec![[0u16;4];values.len()];let mut result=vec![[0f64;3];values.len()];
 separation.transform_pixels(&values,&mut device);appearance.transform_pixels(&device,&mut result);
 let mut points=Vec::with_capacity(result.len());
 for v in result{
  let l=CIEXYZ{X:v[0],Y:v[1],Z:v[2]}.to_lab(&white);
  if ![l.L,l.a,l.b].iter().all(|v|v.is_finite()){return Err("Non-finite Lab result".into())}
  points.push([l.L as f32,l.a as f32,l.b as f32]);
 }
 // Duplicate projected colors are common when targets lie outside the gamut.
 // Quantize only the display point keys, retain the original Lab coordinates.
 let mut seen=std::collections::HashSet::new();
 points.retain(|p|seen.insert(p.map(|v|(v*4.).round() as i32)));
 Ok(Cloud{points,sampled:values.len()})
}
pub fn overlay(reference:&Path,candidate:Option<&Path>,grid:usize)->Overlay{
 Overlay{reference:sample(reference,grid),candidate:candidate.map(|p|sample(p,grid))}
}
/// Equal Lab scaling, orthographic projection. x=a*, y=b*, z=L*-50.
pub fn project(lab:[f32;3],yaw:f32,pitch:f32)->[f32;3]{
 let [l,a,b]=lab;let z=l-50.;
 let x=a*yaw.cos()-b*yaw.sin();let y=a*yaw.sin()+b*yaw.cos();
 [x,z*pitch.cos()-y*pitch.sin(),z*pitch.sin()+y*pitch.cos()]
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn grid_endpoints(){let t=targets(9);assert_eq!(t.len(),729);assert_eq!(t[0],[0.,-128.,-128.]);assert_eq!(t[728],[100.,127.,127.]);}
 #[test]fn projection_origin(){assert_eq!(project([50.,0.,0.],0.7,0.5),[0.,0.,0.]);}
 #[test]fn rotation_preserves_length(){let q=project([80.,30.,-20.],1.2,0.8);assert!((q.iter().map(|v|v*v).sum::<f32>()-2200.).abs()<0.01);}
 #[test]fn rejects_rgb(){let d=tempfile::tempdir().unwrap();let p=d.path().join("rgb.icc");std::fs::write(&p,Profile::new_srgb().icc().unwrap()).unwrap();assert!(sample(&p,9).is_err());}
}
