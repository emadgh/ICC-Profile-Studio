//! ICC DeviceLink generation and profile-level numeric validation.
use std::path::Path;
use lcms2::{Profile,Transform,PixelFormat,Intent,Flags};
type R<T>=Result<T,String>;
fn err(e:impl std::fmt::Display)->String{e.to_string()}
#[derive(Debug,Clone)]
pub struct DeviceLinkReport{pub sample_count:usize,pub max_channel_difference:f64,pub peak_tac:f64,pub maxima:[f64;4]}
pub fn device_link(reference:&Path,candidate:&Path,destination:&Path,intent:Intent)->R<DeviceLinkReport>{
 let a=crate::comparison::open_printer(reference)?;let b=crate::comparison::open_printer(candidate)?;
 let transform:Transform<[u16;4],[u16;4]>=Transform::new(&a,PixelFormat::CMYK_16,&b,PixelFormat::CMYK_16,intent).map_err(err)?;
 let link=Profile::new_device_link(&transform,4.3,Flags::HIGHRES_PRECALC).map_err(err)?;
 let bytes=link.icc().map_err(err)?;
 if bytes.len()<128||&bytes[12..16]!=b"link"||&bytes[16..24]!=b"CMYKCMYK"{return Err("Unexpected DeviceLink class or color spaces".into())}
 let check=Profile::new_icc(&bytes).map_err(err)?;
 let direct:Transform<[u16;4],[u16;4]>=Transform::new_multiprofile(&[&check],PixelFormat::CMYK_16,PixelFormat::CMYK_16,intent,Flags::NO_OPTIMIZE).map_err(err)?;
 let mut samples=Vec::new();
 for i in 0..9usize.pow(4){let mut q=i;let mut v=[0u16;4];for j in (0..4).rev(){v[j]=((q%9)*65535/8) as u16;q/=9;}samples.push(v);}
 let mut expected=vec![[0u16;4];samples.len()];let mut actual=expected.clone();
 transform.transform_pixels(&samples,&mut expected);direct.transform_pixels(&samples,&mut actual);
 let mut report=DeviceLinkReport{sample_count:samples.len(),max_channel_difference:0.,peak_tac:0.,maxima:[0.;4]};
 for (a,b) in expected.iter().zip(actual){
  let total=b.iter().map(|&v|100.*v as f64/65535.).sum::<f64>();report.peak_tac=report.peak_tac.max(total);
  for j in 0..4{report.maxima[j]=report.maxima[j].max(100.*b[j] as f64/65535.);report.max_channel_difference=report.max_channel_difference.max(100.*a[j].abs_diff(b[j]) as f64/65535.);}
 }
 let mut staged=tempfile::NamedTempFile::new_in(destination.parent().unwrap_or(Path::new("."))).map_err(err)?;
 use std::io::Write;staged.write_all(&bytes).map_err(err)?;staged.as_file().sync_all().map_err(err)?;
 staged.persist_noclobber(destination).map_err(err)?;Ok(report)
}

#[cfg(test)]mod tests{
 use super::*;
 #[test]fn native_device_link_samples(){
 let d=tempfile::tempdir().unwrap();let a=d.path().join("a.icc");let b=d.path().join("b.icc");let out=d.path().join("link.icc");
 crate::synthetic_cmyk_v4(false).save_new(&a).unwrap();crate::synthetic_cmyk_v4(true).save_new(&b).unwrap();
 let r=device_link(&a,&b,&out,Intent::RelativeColorimetric).unwrap();
 assert_eq!(r.sample_count,6561);assert!(r.maxima[3]<=0.01);assert!(r.max_channel_difference.is_finite());
 assert!(device_link(&a,&b,&out,Intent::RelativeColorimetric).is_err());
 }
}

// Dynamic links use native 16-bit input/output buffers and an explicit
// preflight CLUT budget. The original CMYK API remains available.
pub const MAX_LINK_VALUES:usize=2_000_000;
#[derive(Debug,Clone,Copy)]
pub struct LinkSettings{pub grid:usize,pub max_ink_pp:f64,pub intent:Intent}
#[derive(Debug,Clone)]
pub struct MultiLinkReport{
 pub source_space:String,pub destination_space:String,
 pub source_channels:Vec<crate::comparison::ChannelInfo>,pub destination_channels:Vec<crate::comparison::ChannelInfo>,
 pub grid:usize,pub nodes:usize,pub byte_count:usize,pub sample_count:usize,pub intent:Intent,
 pub max_channel_difference:f64,pub max_tac_difference:f64,pub peak_tac:f64,pub maxima:Vec<f64>,
 pub mean_de:f64,pub max_de:f64,pub p95_de:f64,pub generated_colorant_tables:usize,
}
pub fn grid_cost(inputs:usize,outputs:usize,grid:usize)->R<usize>{
 if !(2..=15).contains(&inputs)||!(2..=15).contains(&outputs)||!(2..=33).contains(&grid){return Err("Invalid DeviceLink channel count or grid".into())}
 grid.checked_pow(inputs as u32).and_then(|n|n.checked_mul(outputs)).filter(|&n|n<=MAX_LINK_VALUES).ok_or_else(||"DeviceLink CLUT exceeds 2,000,000 values; reduce grid density.".into())
}
pub fn recommended_grid(inputs:usize,outputs:usize)->Option<usize>{
 [33,25,17,13,9,7,5,4,3,2].into_iter().find(|&g|grid_cost(inputs,outputs,g).is_ok())
}
fn link_flags(grid:usize)->Flags{
 // cmsFLAGS_GRIDPOINTS(n) encodes the requested grid in bits 16..23.
 // lcms2 exposes the flag word but does not provide a named grid helper.
 let mut flags=Flags::FORCE_CLUT|Flags::KEEP_SEQUENCE|Flags::NO_WHITE_ON_WHITE_FIXUP;
 flags.0|=(grid as u32)<<16;flags
}
fn colorant_table(p:&crate::Profile,native:&Profile,n:usize)->R<(Vec<u8>,bool)>{
 // A DeviceLink colorant table is Lab encoded. Preserve a valid v4 Lab
 // table exactly; otherwise construct only a *predicted* single-ink
 // table using native D50 relative appearance and existing names.
 if p.version_major()==4&&p.pcs()=="Lab "{
  if let Some(t)=p.tags.get(b"clrt"){
   if t.starts_with(b"clrt")&&crate::u32at(t,8).ok()==Some(n as u32)&&t.len()==12+38*n&&p.colorants().iter().all(Option::is_some){return Ok((t.clone(),false))}
  }
 }
 use lcms2::{CIEXYZ,CIEXYZExt};
 let fmt=crate::print_space::for_profile(native)?;
 let xyz=Profile::new_xyz();
 let to:Transform<u8,[f64;3]>=Transform::new_multiprofile(&[native,&xyz],fmt.pixel,PixelFormat::XYZ_DBL,Intent::RelativeColorimetric,Flags::NO_OPTIMIZE).map_err(err)?;
 let mut samples=vec![0u8;n*n*2];
 for j in 0..n{let i=(j*n+j)*2;samples[i..i+2].copy_from_slice(&65535u16.to_ne_bytes());}
 let mut labs=vec![[0.;3];n];to.transform_pixels(&samples,&mut labs);
 let white=CIEXYZ{X:0.9642,Y:1.,Z:0.8249};
 let names=p.channels();
 let mut table=b"clrt\0\0\0\0".to_vec();crate::put32(&mut table,n as u32);
 for (j,x) in labs.into_iter().enumerate(){
  let lab=CIEXYZ{X:x[0],Y:x[1],Z:x[2]}.to_lab(&white);
  if [lab.L,lab.a,lab.b].iter().any(|v|!v.is_finite()){return Err("Invalid predicted ink colorant PCS".into())}
  // ICC colorant names are ASCII, 31 visible bytes plus terminator.
  let mut name:Vec<u8>=names.get(j).map(String::as_str).unwrap_or("").chars().map(|c|if c.is_ascii()&&!c.is_control(){c as u8}else{b'_'}).take(31).collect();
  if name.is_empty(){name=format!("Channel {}",j+1).into_bytes();}
  name.resize(32,0);table.extend(name);
  for v in [lab.L/100.,(lab.a+128.)/255.,(lab.b+128.)/255.]{crate::put16(&mut table,(v.clamp(0.,1.)*65535.).round() as u16);}
 }
 Ok((table,true))
}
fn parse_grid(p:&crate::Profile,inputs:usize,outputs:usize,requested:usize)->R<usize>{
 let t=p.tags.get(b"A2B0").ok_or("DeviceLink missing A2B0")?;
 if !t.starts_with(b"mAB ")||t.len()<32||t[8] as usize!=inputs||t[9] as usize!=outputs{return Err("Expected v4 DeviceLink mAB dimensions".into())}
 let o=crate::u32at(t,24)? as usize;
 let data=t.get(o..o.checked_add(20).ok_or("Invalid CLUT offset")?).ok_or("Invalid DeviceLink CLUT offset")?;
 if o<32||data[16]!=2||data[..inputs].iter().any(|&v|v as usize!=requested){return Err("Native DeviceLink did not preserve requested 16-bit grid".into())}
 let values=grid_cost(inputs,outputs,requested)?;
 if t.len()<o+20+2*values{return Err("Truncated DeviceLink CLUT".into())}
 Ok(values/outputs)
}
pub fn device_link_multi(reference:&Path,candidate:&Path,destination:&Path,settings:LinkSettings)->R<MultiLinkReport>{
 use lcms2::{CIEXYZ,CIEXYZExt,CIELabExt};
 if !settings.max_ink_pp.is_finite()||!(0.1..=10.).contains(&settings.max_ink_pp){return Err("DeviceLink channel tolerance must be 0.1–10 percentage points".into())}
 let src=crate::Profile::read(reference)?;let dst=crate::Profile::read(candidate)?;
 let a=Profile::new_icc(&src.bytes()?).map_err(err)?;let b=Profile::new_icc(&dst.bytes()?).map_err(err)?;
 let info=|p:&crate::Profile|{let names=p.channels();let standard=p.standard_channels();(0..names.len()).map(|j|crate::comparison::ChannelInfo{name:names[j].clone(),standard:standard[j].clone(),color:p.channel_swatch(j,true),standard_color:p.channel_swatch(j,false)}).collect::<Vec<_>>()};
 let source_channels=info(&src);let destination_channels=info(&dst);
 let fi=crate::print_space::for_profile(&a)?;let fo=crate::print_space::for_profile(&b)?;
 let ni=fi.channels;let no=fo.channels;
 if source_channels.len()!=ni||destination_channels.len()!=no{return Err("DeviceLink channel metadata does not match native formats".into())}
 let grid=if settings.grid==0{recommended_grid(ni,no).ok_or("No grid fits the DeviceLink budget")?}else{settings.grid};
 grid_cost(ni,no,grid)?;
 let direct:Transform<u8,u8>=Transform::new_multiprofile(&[&a,&b],fi.pixel,fo.pixel,settings.intent,Flags::NO_OPTIMIZE|Flags::KEEP_SEQUENCE).map_err(err)?;
 let link=Profile::new_device_link(&direct,4.3,link_flags(grid)).map_err(err)?;
 let bytes=link.icc().map_err(err)?;
 if bytes.len()>32*1024*1024{return Err("DeviceLink exceeds 32 MiB".into())}
 if bytes.len()<128||&bytes[12..16]!=b"link"||bytes[16..20]!=src.header[16..20]||bytes[20..24]!=dst.header[16..20]{return Err("Unexpected DeviceLink class or color spaces".into())}
 let mut parsed=crate::Profile::parse(&bytes)?;
 if parsed.version_major()!=4{return Err("Expected ICC v4 DeviceLink".into())}
 let nodes=parse_grid(&parsed,ni,no,grid)?;
 if !parsed.tags.contains_key(b"pseq"){return Err("DeviceLink missing profile sequence description".into())}
 // Native colorant-table propagation can be absent or incorrect. Retain
 // device order and source labels explicitly for both non-CMYK endpoints.
 let mut generated=0;
 if !src.cmyk(){let (t,derived)=colorant_table(&src,&a,ni)?;parsed.tags.insert(*b"clrt",t);if derived{generated+=1;}}
 if !dst.cmyk(){let (t,derived)=colorant_table(&dst,&b,no)?;parsed.tags.insert(*b"clot",t);if derived{generated+=1;}}
 // Header intent describes the fixed conversion baked into the link.
 parsed.header[64..68].copy_from_slice(&(settings.intent as u32).to_be_bytes());
 parsed.describe(&format!("{} to {} DeviceLink",src.space(),dst.space()));
 let bytes=parsed.bytes()?;
 let check=Profile::new_icc(&bytes).map_err(err)?;
 let actual:Transform<u8,u8>=Transform::new_multiprofile(&[&check],fi.pixel,fo.pixel,settings.intent,Flags::NO_OPTIMIZE).map_err(err)?;
 let samples=crate::editing::device_samples(ni);let count=samples.len()/(ni*2);
 let stride=no*2;
 let mut expected=vec![0u8;count*stride];let mut observed=expected.clone();
 direct.transform_pixels(&samples,&mut expected);actual.transform_pixels(&samples,&mut observed);
 let mut report=MultiLinkReport{source_space:src.space(),destination_space:dst.space(),source_channels,destination_channels,grid,nodes,byte_count:bytes.len(),sample_count:count,intent:settings.intent,max_channel_difference:0.,max_tac_difference:0.,peak_tac:0.,maxima:vec![0.;no],mean_de:0.,max_de:0.,p95_de:0.,generated_colorant_tables:generated};
 let caps=crate::print_space::bounds(&dst).ok();
 let value=|p:&[u8],j:usize|u16::from_ne_bytes([p[2*j],p[2*j+1]]);
 for (x,y) in expected.chunks_exact(stride).zip(observed.chunks_exact(stride)){
  let(mut sx,mut sy)=(0u32,0u32);
  for j in 0..no{
   let a=value(x,j);let b=value(y,j);
   if caps.as_ref().is_some_and(|c|c.channels[j]==0.)&&b!=0{return Err(format!("DeviceLink reopened closed destination channel {}. No file saved.",j+1))}
   report.max_channel_difference=report.max_channel_difference.max(a.abs_diff(b) as f64*100./65535.);
   report.maxima[j]=report.maxima[j].max(b as f64*100./65535.);sx+=u32::from(a);sy+=u32::from(b);
  }
  report.peak_tac=report.peak_tac.max(sy as f64*100./65535.);
  report.max_tac_difference=report.max_tac_difference.max(sx.abs_diff(sy) as f64*100./65535.);
 }
 if report.max_channel_difference>settings.max_ink_pp{return Err(format!("DeviceLink sampled channel deviation {:.3} pp exceeds {:.3} pp. Increase grid if feasible; no file saved.",report.max_channel_difference,settings.max_ink_pp))}
 // Evaluate both output recipes through the same destination A2B model.
 // This isolates approximation of the link, not accuracy to source print.
 let xyz=Profile::new_xyz();let white=CIEXYZ{X:0.9642,Y:1.,Z:0.8249};
 let appearance:Transform<u8,[f64;3]>=Transform::new_multiprofile(&[&b,&xyz],fo.pixel,PixelFormat::XYZ_DBL,Intent::RelativeColorimetric,Flags::NO_OPTIMIZE).map_err(err)?;
 let mut xa=vec![[0.;3];count];let mut xb=xa.clone();
 appearance.transform_pixels(&expected,&mut xa);appearance.transform_pixels(&observed,&mut xb);
 let mut distances=Vec::with_capacity(count);
 for (x,y) in xa.into_iter().zip(xb){
  let lab=|v:[f64;3]|CIEXYZ{X:v[0],Y:v[1],Z:v[2]}.to_lab(&white);
  let d=lab(x).cie2000_delta_e(&lab(y),1.,1.,1.);
  if !d.is_finite()||d<0.{return Err("Invalid DeviceLink predicted color difference".into())}
  distances.push(d);
 }
 distances.sort_by(f64::total_cmp);report.mean_de=distances.iter().sum::<f64>()/count as f64;report.max_de=distances[count-1];report.p95_de=distances[((count as f64*0.95).ceil() as usize)-1];
 let mut staged=tempfile::NamedTempFile::new_in(destination.parent().filter(|p|!p.as_os_str().is_empty()).unwrap_or(Path::new("."))).map_err(err)?;
 use std::io::Write;staged.write_all(&bytes).map_err(err)?;staged.as_file().sync_all().map_err(err)?;
 staged.persist_noclobber(destination).map_err(err)?;Ok(report)
}
#[cfg(test)]
mod multi_tests{
 use super::*;
 #[test]fn bounded_grid_cost(){
  assert_eq!(grid_cost(4,6,17).unwrap(),501126);
  assert!(grid_cost(6,4,33).is_err());assert!(grid_cost(15,15,2).is_ok());
  assert!(grid_cost(15,15,3).is_err());assert!(grid_cost(1,4,2).is_err());
  assert_eq!(recommended_grid(15,15),Some(2));
 }
 #[test]fn mixed_device_link_roundtrip(){
  let d=tempfile::tempdir().unwrap();let a=d.path().join("a.icc");let b=d.path().join("b.icc");
  crate::synthetic_cmyk_v4(true).save_new(&a).unwrap();crate::print_space::fixture(6).save_new(&b).unwrap();
  for (source,dest,grid,nin,nout) in [(&a,&b,9,4,6),(&b,&a,5,6,4),(&b,&b,5,6,6)]{
   let out=d.path().join(format!("link-{nin}-{nout}.icc"));
   let settings=LinkSettings{grid,max_ink_pp:10.,intent:Intent::RelativeColorimetric};
   let r=device_link_multi(source,dest,&out,settings).unwrap();
   assert_eq!(r.source_channels.len(),nin);assert_eq!(r.maxima.len(),nout);
   assert_eq!(r.nodes,grid.pow(nin as u32));assert!(r.sample_count>4096);assert!(r.mean_de.is_finite());
   assert_eq!(r.maxima[nout-1],0.);
   let p=crate::Profile::read(&out).unwrap();
   if nin==6{assert_eq!(crate::u32at(&p.tags[b"clrt"],8).unwrap(),6);}
   if nout==6{assert_eq!(crate::u32at(&p.tags[b"clot"],8).unwrap(),6);}
   assert!(device_link_multi(source,dest,&out,settings).is_err());
  }
 }
 #[test]fn invalid_grid_does_not_save(){
  let d=tempfile::tempdir().unwrap();let p=d.path().join("six.icc");let out=d.path().join("link.icc");
  crate::print_space::fixture(6).save_new(&p).unwrap();
  assert!(device_link_multi(&p,&p,&out,LinkSettings{grid:33,max_ink_pp:1.,intent:Intent::RelativeColorimetric}).is_err());assert!(!out.exists());
 }
}
