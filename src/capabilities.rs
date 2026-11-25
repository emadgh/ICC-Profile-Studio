//! Structural/native preflight. Eligibility is not proof of output accuracy.
use crate::{Profile,R,print_space,editing,multichannel,Lut};
use lcms2::{Tag,TagSignature,Intent};
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Tool{Analysis,Rebuild,Gamma,Reorder,Correction,Version,DeviceLink,Target,Fit}
impl Tool{
 pub fn label(self)->&'static str{match self{Self::Analysis=>"Comparison / Gamut / Gradient",Self::Rebuild=>"Profile Rebuilder",Self::Gamma=>"Tone Curves / Gamma",Self::Reorder=>"Channel Reorder",Self::Correction=>"Bounded curves / correction / smoothing / black / optimization",Self::Version=>"ICC Version Conversion",Self::DeviceLink=>"DeviceLink Generator (source)",Self::Target=>"Profiling Lab / print target",Self::Fit=>"Build ICC from measurements"}}
}
#[derive(Clone,Debug)]
pub struct Capabilities{pub channels:usize,pub tools:Vec<(Tool,R<()>)>}
impl Capabilities{
 pub fn reason(&self,tool:Tool)->Option<&str>{self.tools.iter().find(|(t,_)|*t==tool).and_then(|(_,v)|v.as_ref().err().map(String::as_str))}
}
fn require(ok:bool,why:&str)->R<()>{if ok{Ok(())}else{Err(why.into())}}
fn pipeline(native:&lcms2::Profile,tag:TagSignature,inputs:usize,outputs:usize)->R<()>{
 match native.read_tag(tag){Tag::Pipeline(p) if p.input_channels() as usize==inputs&&p.output_channels() as usize==outputs=>Ok(()),_=>Err(format!("Missing or incompatible native {:?} pipeline",tag))}
}
pub fn inspect(p:&Profile)->Capabilities{
 let n=print_space::format(&p.header[16..20]).map(|f|f.channels).unwrap_or(0);
 let native=p.bytes().and_then(|bytes|lcms2::Profile::new_icc(&bytes).map_err(|e|format!("LittleCMS: {e}")));
 let structural=editing::editable_channel_count(p).map(|_|());
 let decoded=crate::advanced_lut::inspect(p);
 let get_native=||native.as_ref().map_err(Clone::clone);
 let analysis:R<()>=(||{let q=get_native()?;print_space::for_profile(q)?;print_space::Roundtrip::new(q,Intent::RelativeColorimetric)?;Ok(())})();
 let rebuild:R<()>=(||{multichannel::channel_count(p)?;let q=get_native()?;pipeline(q,TagSignature::AToB1Tag,n,3)?;if !p.cmyk(){pipeline(q,TagSignature::BToA1Tag,3,n)?;}Ok(())})();
 let gamma:R<()>=(||{structural.clone()?;get_native()?;require(p.tags.contains_key(b"B2A0"),"Missing B2A0")?;for (k,v) in &decoded{if k.starts_with('B'){let s=v.as_ref().map_err(Clone::clone)?;require(s.inputs==3&&s.outputs==n,"Reverse LUT channel mismatch")?;require(s.device_curves.iter().all(|(_,v)|v.is_ok()),"Unsupported device curve")?;}}Ok(())})();
 let reorder:R<()>=(||{structural.clone()?;get_native()?;editing::supported_tags(p)?;require(!p.tags.contains_key(b"clro"),"Colorant order tag requires a mapping adapter")?;require(p.tags.contains_key(b"A2B0")&&p.tags.contains_key(b"B2A0"),"Both A2B0 and B2A0 are required")?;for (k,v) in &decoded{let s=v.as_ref().map_err(Clone::clone)?;let dims=if k.starts_with('A'){(n,3)}else{(3,n)};require((s.inputs,s.outputs)==dims,"LUT channel mismatch")?;}Ok(())})();
 let correction:R<()>=(||{rebuild.clone()?;require(p.version_major()==4&&p.pcs()=="Lab ","Requires ICC v4 Lab PCS")?;print_space::bounds(p)?;let q=get_native()?;for (key,sig) in [(b"B2A0",TagSignature::BToA0Tag),(b"B2A1",TagSignature::BToA1Tag),(b"B2A2",TagSignature::BToA2Tag)]{require(p.tags.get(key).is_some_and(|v|v.starts_with(b"mBA ")),"Requires three supported mBA reverse LUTs")?;pipeline(q,sig,3,n)?;}Ok(())})();
 let version:R<()>=(||{structural.clone()?;get_native()?;require(p.pcs()=="Lab ","Requires Lab PCS")?;require(p.tags.contains_key(b"wtpt")&&p.tags.contains_key(b"desc"),"Missing media white or profile description")?;for key in [b"A2B0",b"A2B1",b"A2B2",b"B2A0",b"B2A1",b"B2A2"]{let raw=p.tags.get(key).ok_or("All six intent tags are required")?;let f=key[0]==b'A';let expected=if p.version_major()==2{b"mft2"}else if f{b"mAB "}else{b"mBA "};require(raw.starts_with(expected),"Unsupported version conversion LUT type")?;let l=Lut::parse(raw)?;require((l.inputs,l.outputs)==if f{(n,3)}else{(3,n)},"LUT channel mismatch")?;}Ok(())})();
 let target:R<()>=(||{structural.clone()?;analysis.clone()?;Ok(())})();
 let fit:R<()>=(||{target.clone()?;require(p.cmyk(),"N-color fitting is not supported by the current Argyll colprof workflow")})();
 Capabilities{channels:n,tools:vec![(Tool::Analysis,analysis.clone()),(Tool::Rebuild,rebuild),(Tool::Gamma,gamma),(Tool::Reorder,reorder),(Tool::Correction,correction),(Tool::Version,version),(Tool::DeviceLink,analysis),(Tool::Target,target),(Tool::Fit,fit)]}
}
/// Display-only single-ink predictions, not measured spectra or ink identities.
pub fn predicted_swatches(p:&Profile)->R<Vec<u32>>{
 use lcms2::{Transform,PixelFormat,CIEXYZ,CIEXYZExt};
 let native=lcms2::Profile::new_icc(&p.bytes()?).map_err(|e|e.to_string())?;
 let f=print_space::for_profile(&native)?;
 let xyz=lcms2::Profile::new_xyz();
 let transform:Transform<u8,[f64;3]>=Transform::new(&native,f.pixel,&xyz,PixelFormat::XYZ_DBL,Intent::RelativeColorimetric).map_err(|e|e.to_string())?;
 let n=f.channels;let mut input=vec![0u8;n*n*2];
 for j in 0..n{let pos=(j*n+j)*2;input[pos..pos+2].copy_from_slice(&u16::MAX.to_ne_bytes());}
 let mut output=vec![[0.;3];n];transform.transform_pixels(&input,&mut output);
 let white=CIEXYZ{X:0.9642,Y:1.,Z:0.8249};
 output.into_iter().map(|v|{require(v.iter().all(|x|x.is_finite()),"Invalid single-ink prediction")?;let l=CIEXYZ{X:v[0],Y:v[1],Z:v[2]}.to_lab(&white);let [r,g,b]=crate::gamut_raster::lab_rgb([l.L as f32,l.a as f32,l.b as f32]);Ok((u32::from(r)<<16)|(u32::from(g)<<8)|u32::from(b))}).collect()
}
#[cfg(test)]
mod tests{
 use super::*;
 #[test]fn native_count_and_preflight_matrix(){
  for n in [3,5,6,7,8,15]{let p=print_space::fixture(n);let c=inspect(&p);assert_eq!(c.channels,n);assert!(c.reason(Tool::Rebuild).is_none(),"{n}: {:?}",c.reason(Tool::Rebuild));assert!(c.reason(Tool::Correction).is_none());assert!(c.reason(Tool::Fit).is_some());assert_eq!(predicted_swatches(&p).unwrap().len(),n);}
 }
 #[test]fn missing_reverse_disables_rebuild_and_correction(){let mut p=print_space::fixture(6);p.tags.remove(b"B2A1");let c=inspect(&p);assert!(c.reason(Tool::Rebuild).is_some());assert!(c.reason(Tool::Correction).is_some());assert!(c.reason(Tool::Version).is_some());}
 #[test]fn cmyk_shared_rebuild(){let p=crate::synthetic_cmyk_v4(true);let caps=[0.6,0.6,0.6,0.];let out=multichannel::rebuild(&p,&caps,1.8,9,|_|{}).unwrap();let v=multichannel::verify(&out,&caps,1.8).unwrap();assert_eq!(v.channels[3],0.);assert_eq!(out.space(),"CMYK");}
 #[test]#[ignore="Requires operator-supplied licensed real printer profiles"]
 fn real_profile_corpus(){
  let dir=std::env::var("ICC_REAL_PROFILE_DIR").expect("Set ICC_REAL_PROFILE_DIR");
  let mut found=std::collections::BTreeSet::new();
  for entry in std::fs::read_dir(dir).unwrap(){let path=entry.unwrap().path();if !path.extension().is_some_and(|x|x.eq_ignore_ascii_case("icc")||x.eq_ignore_ascii_case("icm")){continue}
   let p=Profile::read(&path).unwrap();let c=inspect(&p);println!("{} | {} | {:?}",path.display(),p.space(),c.tools);found.insert(c.channels);
   if c.reason(Tool::Rebuild).is_none(){let mut caps=vec![1.;c.channels];caps[c.channels-1]=0.;let out=multichannel::rebuild(&p,&caps,2.0,9,|_|{}).unwrap();multichannel::verify(&out,&caps,2.0).unwrap();assert_eq!(out.space(),p.space());}
  }
  for n in [3,5,6,7,8,15]{assert!(found.contains(&n),"Missing real {n}-channel sample");}
 }
}
