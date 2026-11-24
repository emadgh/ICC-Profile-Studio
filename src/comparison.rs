//! Compare predicted color and separations using LittleCMS 2, not display RGB distances.
use lcms2::{Profile, PixelFormat, Transform, Intent, CIEXYZ, CIEXYZExt, CIELab, CIELabExt, ColorSpaceSignature, ProfileClassSignature};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct InkStats { pub mean:[f64;4],pub max:[f64;4],pub mean_tac:f64,pub max_tac:f64 }
#[derive(Debug, Clone)]
pub struct Comparison {
 pub count:usize,pub mean_de:f64,pub p95_de:f64,pub max_de:f64,pub worst_rgb:[u8;3],
 pub reference:InkStats,pub candidate:InkStats,pub neutral:Vec<NeutralSample>,
 pub histogram:[usize;5],
}
#[derive(Debug, Clone)]
pub struct NeutralSample {pub rgb:u8,pub reference_lab:[f64;3],pub candidate_lab:[f64;3],pub delta_e:f64,pub reference_ink:[f64;4],pub candidate_ink:[f64;4]}
pub(crate) fn open_printer(p:&Path)->Result<Profile,String>{
 let meta=std::fs::metadata(p).map_err(|e|e.to_string())?;
 if meta.len()>128*1024*1024{return Err("ICC exceeds 128 MiB".into())}
 let profile=Profile::new_icc(&std::fs::read(p).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
 if profile.color_space()!=ColorSpaceSignature::CmykData||profile.device_class()!=ProfileClassSignature::OutputClass{return Err("Comparison requires CMYK output/printer profiles.".into())}
 Ok(profile)
}
fn percentages(pixel:[u16;4])->[f64;4]{pixel.map(|v|v as f64*100./65535.)}
fn ink_stats(pixels:&[[u16;4]])->InkStats{
 let mut out=InkStats{mean:[0.;4],max:[0.;4],mean_tac:0.,max_tac:0.};
 for &p in pixels{
  let v=percentages(p);
  for j in 0..4{out.mean[j]+=v[j];out.max[j]=out.max[j].max(v[j]);}
  let tac=v.iter().sum::<f64>();out.mean_tac+=tac;out.max_tac=out.max_tac.max(tac);
 }
 for x in &mut out.mean{*x/=pixels.len() as f64;}
 out.mean_tac/=pixels.len() as f64;out
}
pub(crate) fn simulate(profile:&Profile,samples:&[[u8;3]],intent:Intent)->Result<(Vec<[u16;4]>,Vec<CIELab>),String>{
 let srgb=Profile::new_srgb();let xyz=Profile::new_xyz();
 let to_device:Transform<[u8;3],[u16;4]>=Transform::new(&srgb,PixelFormat::RGB_8,profile,PixelFormat::CMYK_16,intent).map_err(|e|e.to_string())?;
 // Both output appearances are evaluated in media-relative D50 PCS.
 let to_xyz:Transform<[u16;4],[f64;3]>=Transform::new(profile,PixelFormat::CMYK_16,&xyz,PixelFormat::XYZ_DBL,Intent::RelativeColorimetric).map_err(|e|e.to_string())?;
 let mut device=vec![[0u16;4];samples.len()];let mut values=vec![[0f64;3];samples.len()];
 to_device.transform_pixels(samples,&mut device);to_xyz.transform_pixels(&device,&mut values);
 let white=CIEXYZ{X:0.9642,Y:1.,Z:0.8249};
 let lab:Vec<_>=values.into_iter().map(|p|CIEXYZ{X:p[0],Y:p[1],Z:p[2]}.to_lab(&white)).collect();
 if lab.iter().any(|p|!p.L.is_finite()||!p.a.is_finite()||!p.b.is_finite()){return Err("Non-finite PCS output".into())}
 Ok((device,lab))
}
pub fn compare(reference:&Path,candidate:&Path,grid:usize,intent:Intent)->Result<Comparison,String>{
 if !(3..=33).contains(&grid){return Err("Grid must be 3..33".into())}
 let reference=open_printer(reference)?;let candidate=open_printer(candidate)?;
 let mut samples=Vec::with_capacity(grid.pow(3)+33);
 for r in 0..grid{for g in 0..grid{for b in 0..grid{
  samples.push([r,g,b].map(|i|((i*255+(grid-1)/2)/(grid-1)) as u8));
 }}}
 let count=samples.len();for i in 0..33{samples.push([((i*255+16)/32) as u8;3]);}
 let (a,la)=simulate(&reference,&samples,intent)?;let (b,lb)=simulate(&candidate,&samples,intent)?;
 let mut distances:Vec<f64>=la.iter().zip(&lb).map(|(x,y)|x.cie2000_delta_e(y,1.,1.,1.)).collect();
 if distances.iter().any(|v|!v.is_finite()||*v<0.){return Err("Invalid Delta E result".into())}
 let neutral=(count..samples.len()).map(|i|NeutralSample{rgb:samples[i][0],reference_lab:[la[i].L,la[i].a,la[i].b],candidate_lab:[lb[i].L,lb[i].a,lb[i].b],delta_e:distances[i],reference_ink:percentages(a[i]),candidate_ink:percentages(b[i])}).collect();
 distances.truncate(count);
 let worst=distances.iter().enumerate().max_by(|(_,a),(_,b)|a.total_cmp(b)).map(|(i,_)|i).unwrap();
 let worst_rgb=samples[worst];
 let mean_de=distances.iter().sum::<f64>()/count as f64;
 let mut histogram=[0;5];for &v in &distances{histogram[if v<1.{0}else if v<2.{1}else if v<3.{2}else if v<5.{3}else{4}]+=1;}
 distances.sort_by(f64::total_cmp);
 Ok(Comparison{count,mean_de,p95_de:distances[((count as f64*0.95).ceil() as usize).saturating_sub(1)],max_de:distances[count-1],worst_rgb,
 reference:ink_stats(&a[..count]),candidate:ink_stats(&b[..count]),neutral,histogram})
}
#[cfg(test)]
mod tests{
 use super::*;
 #[test]fn delta_e_reference_pair(){
  let x=CIELab{L:50.,a:2.6772,b:-79.7751};let y=CIELab{L:50.,a:0.,b:-82.7485};
  assert!((x.cie2000_delta_e(&y,1.,1.,1.)-2.0425).abs()<0.0001);
 }
 #[test]fn ink_statistics(){let s=ink_stats(&[[65535,0,0,0],[0,65535,65535,0]]);assert_eq!(s.mean,[50.,50.,50.,0.]);assert_eq!(s.mean_tac,150.);assert_eq!(s.max_tac,200.);}
 #[test]fn rejects_rgb(){let d=tempfile::tempdir().unwrap();let p=d.path().join("rgb.icc");std::fs::write(&p,Profile::new_srgb().icc().unwrap()).unwrap();assert!(compare(&p,&p,9,Intent::RelativeColorimetric).is_err());}
}
