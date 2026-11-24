//! Compare predicted color and separations using LittleCMS 2, not display RGB distances.
use lcms2::{Profile, PixelFormat, Transform, Intent, CIEXYZ, CIEXYZExt, CIELab, CIELabExt, ColorSpaceSignature};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct InkStats { pub mean:Vec<f64>,pub max:Vec<f64>,pub mean_tac:f64,pub max_tac:f64 }
#[derive(Debug, Clone)]
pub struct Comparison {
 pub intent:Intent,pub count:usize,pub mean_de:f64,pub p95_de:f64,pub max_de:f64,pub worst_rgb:[u8;3],
 pub reference:InkStats,pub candidate:InkStats,pub neutral:Vec<NeutralSample>,
 pub histogram:[usize;5],pub reference_channels:Vec<ChannelInfo>,pub candidate_channels:Vec<ChannelInfo>,
}
#[derive(Debug, Clone)]
pub struct NeutralSample {pub rgb:u8,pub reference_lab:[f64;3],pub candidate_lab:[f64;3],pub delta_e:f64,pub reference_ink:Vec<f64>,pub candidate_ink:Vec<f64>}
#[derive(Debug,Clone)]
pub struct ChannelInfo{pub name:String,pub standard:String,pub color:u32,pub standard_color:u32}
impl ChannelInfo{
 pub fn label(&self,embedded:bool)->&str{if embedded{&self.name}else{&self.standard}}
 pub fn swatch(&self,embedded:bool)->u32{if embedded{self.color}else{self.standard_color}}
}
pub(crate) fn open_output(p:&Path)->Result<(Profile,Vec<ChannelInfo>),String>{
 let meta=std::fs::metadata(p).map_err(|e|e.to_string())?;
 if meta.len()>128*1024*1024{return Err("ICC exceeds 128 MiB".into())}
 let data=std::fs::read(p).map_err(|e|e.to_string())?;
 let profile=Profile::new_icc(&data).map_err(|e|e.to_string())?;
 let format=crate::print_space::for_profile(&profile)?;
 let parsed=crate::Profile::parse(&data)?;
 let names=parsed.channels();let standard=parsed.standard_channels();
 if names.len()!=format.channels||standard.len()!=format.channels{return Err("ICC channel metadata does not match native format".into())}
 let channels=(0..format.channels).map(|j|ChannelInfo{name:names[j].clone(),standard:standard[j].clone(),color:parsed.channel_swatch(j,true),standard_color:parsed.channel_swatch(j,false)}).collect();
 Ok((profile,channels))
}
/// Preserve the CMYK-only gate for existing four-channel DeviceLink,
/// recipe and characterization tools. Only analysis calls open_output.
pub(crate) fn open_printer(p:&Path)->Result<Profile,String>{
 let (profile,_)=open_output(p)?;
 if profile.color_space()!=ColorSpaceSignature::CmykData{return Err("This tool requires CMYK output/printer profiles.".into())}
 Ok(profile)
}
fn ink_stats(pixels:&[Vec<f64>],channels:usize)->InkStats{
 let mut out=InkStats{mean:vec![0.;channels],max:vec![0.;channels],mean_tac:0.,max_tac:0.};
 for v in pixels{
  for j in 0..channels{out.mean[j]+=v[j];out.max[j]=out.max[j].max(v[j]);}
  let tac=v.iter().sum::<f64>();out.mean_tac+=tac;out.max_tac=out.max_tac.max(tac);
 }
 let n=pixels.len().max(1) as f64;
 for x in &mut out.mean{*x/=n;}
 out.mean_tac/=n;out
}
pub(crate) fn simulate(profile:&Profile,samples:&[[u8;3]],intent:Intent)->Result<(Vec<Vec<f64>>,Vec<CIELab>),String>{
 let f=crate::print_space::for_profile(profile)?;
 let srgb=Profile::new_srgb();let xyz=Profile::new_xyz();
 let to_device:Transform<[u8;3],u8>=Transform::new(&srgb,PixelFormat::RGB_8,profile,f.pixel,intent).map_err(|e|e.to_string())?;
 // Compare the predicted appearance in media-relative D50 for both printers.
 let to_xyz:Transform<u8,[f64;3]>=Transform::new(profile,f.pixel,&xyz,PixelFormat::XYZ_DBL,Intent::RelativeColorimetric).map_err(|e|e.to_string())?;
 let stride=f.channels*2;
 let mut device=vec![0u8;samples.len().checked_mul(stride).ok_or("Device buffer overflow")?];
 let mut values=vec![[0f64;3];samples.len()];
 to_device.transform_pixels(samples,&mut device);to_xyz.transform_pixels(&device,&mut values);
 let white=CIEXYZ{X:0.9642,Y:1.,Z:0.8249};
 let lab:Vec<_>=values.into_iter().map(|p|CIEXYZ{X:p[0],Y:p[1],Z:p[2]}.to_lab(&white)).collect();
 if lab.iter().any(|p|!p.L.is_finite()||!p.a.is_finite()||!p.b.is_finite()){return Err("Non-finite PCS output".into())}
 let inks=device.chunks_exact(stride).map(|v|v.chunks_exact(2).map(|p|u16::from_ne_bytes([p[0],p[1]]) as f64*100./65535.).collect()).collect();
 Ok((inks,lab))
}
pub fn compare(reference:&Path,candidate:&Path,grid:usize,intent:Intent)->Result<Comparison,String>{
 if !(3..=33).contains(&grid){return Err("Grid must be 3..33".into())}
 let (reference,reference_channels)=open_output(reference)?;let (candidate,candidate_channels)=open_output(candidate)?;
 let mut samples=Vec::with_capacity(grid.pow(3)+33);
 for r in 0..grid{for g in 0..grid{for b in 0..grid{
  samples.push([r,g,b].map(|i|((i*255+(grid-1)/2)/(grid-1)) as u8));
 }}}
 let count=samples.len();for i in 0..33{samples.push([((i*255+16)/32) as u8;3]);}
 let (a,la)=simulate(&reference,&samples,intent)?;let (b,lb)=simulate(&candidate,&samples,intent)?;
 let mut distances:Vec<f64>=la.iter().zip(&lb).map(|(x,y)|x.cie2000_delta_e(y,1.,1.,1.)).collect();
 if distances.iter().any(|v|!v.is_finite()||*v<0.){return Err("Invalid Delta E result".into())}
 let neutral=(count..samples.len()).map(|i|NeutralSample{rgb:samples[i][0],reference_lab:[la[i].L,la[i].a,la[i].b],candidate_lab:[lb[i].L,lb[i].a,lb[i].b],delta_e:distances[i],reference_ink:a[i].clone(),candidate_ink:b[i].clone()}).collect();
 distances.truncate(count);
 let worst=distances.iter().enumerate().max_by(|(_,a),(_,b)|a.total_cmp(b)).map(|(i,_)|i).unwrap();
 let worst_rgb=samples[worst];
 let mean_de=distances.iter().sum::<f64>()/count as f64;
 let mut histogram=[0;5];for &v in &distances{histogram[if v<1.{0}else if v<2.{1}else if v<3.{2}else if v<5.{3}else{4}]+=1;}
 distances.sort_by(f64::total_cmp);
 Ok(Comparison{intent,count,mean_de,p95_de:distances[((count as f64*0.95).ceil() as usize).saturating_sub(1)],max_de:distances[count-1],worst_rgb,
 reference:ink_stats(&a[..count],reference_channels.len()),candidate:ink_stats(&b[..count],candidate_channels.len()),neutral,histogram,reference_channels,candidate_channels})
}
#[cfg(test)]
mod tests{
 use super::*;
 #[test]fn delta_e_reference_pair(){
  let x=CIELab{L:50.,a:2.6772,b:-79.7751};let y=CIELab{L:50.,a:0.,b:-82.7485};
  assert!((x.cie2000_delta_e(&y,1.,1.,1.)-2.0425).abs()<0.0001);
 }
 #[test]fn ink_statistics(){let s=ink_stats(&[vec![100.,0.,0.,0.],vec![0.,100.,100.,0.]],4);assert_eq!(s.mean,[50.,50.,50.,0.]);assert_eq!(s.mean_tac,150.);assert_eq!(s.max_tac,200.);}
 #[test]fn self_comparison_all_intents(){
  let d=tempfile::tempdir().unwrap();let p=d.path().join("fixture.icc");crate::synthetic_cmyk_v4(false).save_new(&p).unwrap();
  for i in [Intent::RelativeColorimetric,Intent::Perceptual,Intent::Saturation,Intent::AbsoluteColorimetric]{
   let r=compare(&p,&p,9,i).unwrap();assert_eq!(r.intent,i);assert_eq!(r.count,729);assert_eq!(r.neutral.len(),33);assert!(r.max_de<1e-8);
  }
 }
 #[test]fn mixed_channel_counts_and_self_comparison(){
  let d=tempfile::tempdir().unwrap();let a=d.path().join("four.icc");let b=d.path().join("six.icc");
  crate::synthetic_cmyk_v4(true).save_new(&a).unwrap();crate::print_space::fixture(6).save_new(&b).unwrap();
  let r=compare(&a,&b,9,Intent::RelativeColorimetric).unwrap();
  assert_eq!(r.reference.mean.len(),4);assert_eq!(r.candidate.mean.len(),6);assert_eq!(r.candidate.max[5],0.);
  assert_eq!(r.reference_channels.len(),4);assert_eq!(r.candidate_channels.len(),6);
  assert!(r.neutral.iter().all(|v|v.reference_ink.len()==4&&v.candidate_ink.len()==6));
  let same=compare(&b,&b,9,Intent::RelativeColorimetric).unwrap();assert!(same.max_de<1e-8);
  assert!(open_printer(&b).is_err());
 }
 #[test]fn rejects_rgb(){let d=tempfile::tempdir().unwrap();let p=d.path().join("rgb.icc");std::fs::write(&p,Profile::new_srgb().icc().unwrap()).unwrap();assert!(compare(&p,&p,9,Intent::RelativeColorimetric).is_err());}
}
