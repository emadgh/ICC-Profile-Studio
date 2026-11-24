//! Production-oriented CMYK TIFF conversion. LittleCMS handles color;
//! image/tiff handle codecs. All job files are staged before publication.
use std::{fs::{self,File},io::{BufReader,BufWriter},path::{Path,PathBuf}};
use image::{ImageDecoder,DynamicImage,ImageBuffer,Rgb};
use lcms2::{Profile,Transform,PixelFormat,Intent,Flags,ColorSpaceSignature};
use tiff::{decoder::{Decoder,DecodingResult},encoder::{TiffEncoder,colortype::CMYK16,Rational},tags::{Tag,ResolutionUnit},ColorType};
type R<T>=Result<T,String>;
fn err(e:impl std::fmt::Display)->String{e.to_string()}
#[derive(Clone,Copy,Debug)]
pub struct Options{pub intent:Intent,pub dpi:u32,pub assume_untagged:bool,pub tac:f64}
impl Default for Options{fn default()->Self{Self{intent:Intent::RelativeColorimetric,dpi:300,assume_untagged:false,tac:400.}}}
#[derive(Debug,Clone)]
pub struct Output{pub directory:PathBuf,pub width:u32,pub height:u32,pub mean:[f64;4],pub max:[f64;4],pub peak_tac:f64,pub over_tac:usize}
enum Pixels{Rgb(Vec<[u16;3]>),Cmyk(Vec<[u16;4]>)}
struct Input{width:u32,height:u32,pixels:Pixels,profile:Profile}
fn check_size(w:u32,h:u32)->R<usize>{
 let n=w as u64*h as u64;if n==0||n>24_000_000{return Err("Preview converter supports up to 24 megapixels; larger jobs require a tiled/streaming implementation.".into())}Ok(n as usize)
}
fn load(path:&Path,reference:&Profile,assume:bool)->R<Input>{
 let reader=image::ImageReader::open(path).map_err(err)?.with_guessed_format().map_err(err)?;
 if reader.format()==Some(image::ImageFormat::Tiff){
  let mut d=Decoder::new(BufReader::new(File::open(path).map_err(err)?)).map_err(err)?;
  if d.more_images(){return Err("Multi-page TIFF is not supported".into())}
  let (width,height)=d.dimensions().map_err(err)?;let count=check_size(width,height)?;
  if d.get_tag_unsigned::<u16>(Tag::Orientation).unwrap_or(1)!=1{return Err("Normalize TIFF orientation before conversion".into())}
  if d.get_tag_unsigned::<u16>(Tag::PlanarConfiguration).unwrap_or(1)!=1{return Err("Planar TIFF not supported; save interleaved TIFF".into())}
  let color=d.colortype().map_err(err)?;
  let cmyk=matches!(color,ColorType::CMYK(8)|ColorType::CMYK(16));
  if !cmyk&&!matches!(color,ColorType::RGB(8)|ColorType::RGB(16)){return Err("TIFF must be RGB or CMYK, 8/16 bit, without alpha".into())}
  let bytes=d.get_tag_u8_vec(Tag::IccProfile).ok();
  let profile=match bytes{Some(b)=>Profile::new_icc(&b).map_err(err)?,None if assume&&cmyk=>Profile::new_icc(&reference.icc().map_err(err)?).map_err(err)?,None if assume=>Profile::new_srgb(),None=>return Err("Untagged image: explicitly enable sRGB / reference CMYK assumption".into())};
  let raw:Vec<u16>=match d.read_image().map_err(err)?{DecodingResult::U8(v)=>v.into_iter().map(|v|v as u16*257).collect(),DecodingResult::U16(v)=>v,_=>return Err("Unsupported TIFF sample depth".into())};
  let pixels=if cmyk{if raw.len()!=count*4||profile.color_space()!=ColorSpaceSignature::CmykData{return Err("CMYK pixels/profile mismatch".into())}Pixels::Cmyk(raw.chunks_exact(4).map(|v|[v[0],v[1],v[2],v[3]]).collect())}
   else{if raw.len()!=count*3||profile.color_space()!=ColorSpaceSignature::RgbData{return Err("RGB pixels/profile mismatch".into())}Pixels::Rgb(raw.chunks_exact(3).map(|v|[v[0],v[1],v[2]]).collect())};
  return Ok(Input{width,height,pixels,profile})
 }
 if !matches!(reader.format(),Some(image::ImageFormat::Png|image::ImageFormat::Jpeg)){return Err("Use RGB PNG/JPEG or RGB/CMYK TIFF".into())}
 let mut decoder=reader.into_decoder().map_err(err)?;
 let (w,h)=decoder.dimensions();check_size(w,h)?;
 if !matches!(decoder.color_type(),image::ColorType::Rgb8|image::ColorType::Rgb16){return Err("RGB only; flatten alpha and expand grayscale first".into())}
 let profile=match decoder.icc_profile().map_err(err)?{Some(b)=>Profile::new_icc(&b).map_err(err)?,None if assume=>Profile::new_srgb(),None=>return Err("Untagged image: explicitly enable sRGB assumption".into())};
 if profile.color_space()!=ColorSpaceSignature::RgbData{return Err("Embedded ICC is not RGB; use TIFF for CMYK input".into())}
 let orientation=decoder.orientation().map_err(err)?;
 let mut img=DynamicImage::from_decoder(decoder).map_err(err)?;img.apply_orientation(orientation);
 let rgb=img.to_rgb16();let (width,height)=rgb.dimensions();
 Ok(Input{width,height,pixels:Pixels::Rgb(rgb.pixels().map(|v|v.0).collect()),profile})
}
fn to_cmyk(input:&Input,dst:&Profile,intent:Intent)->R<Vec<[u16;4]>>{
 let mut out=vec![[0u16;4];input.width as usize*input.height as usize];
 match &input.pixels{
  Pixels::Rgb(v)=>{let t:Transform<[u16;3],[u16;4]>=Transform::new(&input.profile,PixelFormat::RGB_16,dst,PixelFormat::CMYK_16,intent).map_err(err)?;t.transform_pixels(v,&mut out);}
  Pixels::Cmyk(v)=>{let t:Transform<[u16;4],[u16;4]>=Transform::new(&input.profile,PixelFormat::CMYK_16,dst,PixelFormat::CMYK_16,intent).map_err(err)?;t.transform_pixels(v,&mut out);}
 }Ok(out)
}
pub(crate) fn thumb_indices(w:u32,h:u32)->(u32,u32,Vec<usize>){
 let ratio=(640.0/w.max(h) as f64).min(1.);let tw=((w as f64*ratio).round() as u32).max(1);let th=((h as f64*ratio).round() as u32).max(1);
 let mut idx=Vec::with_capacity((tw*th) as usize);for y in 0..th{for x in 0..tw{idx.push(((y as u64*h as u64/th as u64)*w as u64+x as u64*w as u64/tw as u64) as usize);}}
 (tw,th,idx)
}
pub(crate) fn proof(pixels:&[[u16;4]],p:&Profile,idx:&[usize],w:u32,h:u32,path:&Path)->R<()>{
 let srgb=Profile::new_srgb();let t:Transform<[u16;4],[u8;3]>=Transform::new(p,PixelFormat::CMYK_16,&srgb,PixelFormat::RGB_8,Intent::RelativeColorimetric).map_err(err)?;
 let selected:Vec<_>=idx.iter().map(|&i|pixels[i]).collect();let mut rgb=vec![[0u8;3];idx.len()];t.transform_pixels(&selected,&mut rgb);
 ImageBuffer::<Rgb<u8>,_>::from_raw(w,h,rgb.into_iter().flatten().collect::<Vec<_>>()).ok_or("Invalid proof dimensions")?.save(path).map_err(err)
}
pub fn write_cmyk_tiff(path:&Path,w:u32,h:u32,pixels:&[[u16;4]],icc:&[u8],dpi:u32)->R<()>{
 if pixels.len()!=check_size(w,h)?||!(50..=2400).contains(&dpi){return Err("Invalid TIFF size or DPI".into())}
 let mut e=TiffEncoder::new(BufWriter::new(File::create(path).map_err(err)?)).map_err(err)?;
 let mut img=e.new_image::<CMYK16>(w,h).map_err(err)?;
 img.resolution(ResolutionUnit::Inch,Rational{n:dpi,d:1});
 img.encoder().write_tag(Tag::IccProfile,icc).map_err(err)?;
 // Strip-based writing avoids allocating another full flattened image.
 img.rows_per_strip(16).map_err(err)?;
 let mut offset=0;
 while img.next_strip_sample_count()>0{
  let n=img.next_strip_sample_count() as usize/4;
  let values:Vec<u16>=pixels[offset..offset+n].iter().flatten().copied().collect();
  img.write_strip(&values).map_err(err)?;offset+=n;
 }
 img.finish().map_err(err)
}
pub fn convert(source:&Path,reference:&Path,destination:&Path,root:&Path,options:Options)->R<Output>{
 if !options.tac.is_finite()||!(0.0..=400.).contains(&options.tac)||!(50..=2400).contains(&options.dpi){return Err("Invalid options".into())}
 let a=crate::comparison::open_printer(reference)?;let b=crate::comparison::open_printer(destination)?;
 let input=load(source,&a,options.assume_untagged)?;
 let stem=source.file_stem().ok_or("Missing filename")?.to_string_lossy();
 let directory=root.join(format!("{stem}_ICC"));
 if directory.exists(){return Err(format!("Output folder exists: {}",directory.display()))}
 let stage=tempfile::tempdir_in(root).map_err(err)?;
 let p=to_cmyk(&input,&b,options.intent)?;
 let mut mean=[0f64;4];let mut max=[0f64;4];let mut peak_tac=0f64;let mut over_tac=0;
 for v in &p{let c=v.map(|x|100.*x as f64/65535.);let tac=c.iter().sum::<f64>();peak_tac=peak_tac.max(tac);if tac>options.tac+1e-8{over_tac+=1;}for j in 0..4{mean[j]+=c[j];max[j]=max[j].max(c[j]);}}
 for m in &mut mean{*m/=p.len() as f64;}
 if over_tac>0{return Err(format!("{over_tac} pixels exceed chosen TAC ceiling {:.1}% (peak {:.2}%). Rebuild the destination ICC; converter will not clip inks.",options.tac,peak_tac))}
 write_cmyk_tiff(&stage.path().join("converted.tif"),input.width,input.height,&p,&b.icc().map_err(err)?,options.dpi)?;
 let (tw,th,idx)=thumb_indices(input.width,input.height);
 proof(&p,&b,&idx,tw,th,&stage.path().join("after.png"))?;
 for j in 0..4{
  let data=idx.iter().map(|&i|255-(p[i][j] as u32*255/65535) as u8).collect::<Vec<_>>();
  ImageBuffer::<image::Luma<u8>,_>::from_raw(tw,th,data).ok_or("Invalid separation size")?.save(stage.path().join(format!("channel_{}.png",j+1))).map_err(err)?;
 }
 let heat:Vec<u8>=idx.iter().flat_map(|&i|{let t=p[i].iter().map(|&v|v as f64/65535.).sum::<f64>()/4.;[(255.*t) as u8,(255.*(1.-(2.*t-1.).abs())) as u8,(255.*(1.-t)) as u8]}).collect();
 ImageBuffer::<Rgb<u8>,_>::from_raw(tw,th,heat).ok_or("Invalid heatmap")?.save(stage.path().join("coverage.png")).map_err(err)?;
 drop(p);
 let before=to_cmyk(&input,&a,options.intent)?;
 proof(&before,&a,&idx,tw,th,&stage.path().join("before.png"))?;
 fs::write(stage.path().join("report.txt"),format!("Source: {}\nReference: {}\nDestination: {}\n{} x {} pixels; {} DPI; CMYK16\nMean inks {:?}\nMax inks {:?}\nPeak TAC {:.3}%\nPreview is media-relative sRGB simulation, not measured print or calibrated monitor proof. Separation/heatmap PNGs are reduced-size diagnostic previews. TIFF metadata other than embedded ICC and selected DPI is not retained.",source.display(),reference.display(),destination.display(),input.width,input.height,options.dpi,mean,max,peak_tac)).map_err(err)?;
 fs::rename(stage.path(),&directory).map_err(err)?;
 Ok(Output{directory,width:input.width,height:input.height,mean,max,peak_tac,over_tac})
}
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

/// Export monitor-encoded before/after previews, embedded with the exact
/// selected RGB display ICC. Use a color-managed external viewer, not raw GPUI
/// texture display. This is a prediction; the monitor must be calibrated.
#[derive(Clone,Copy,Debug)]
pub struct ProofOptions{pub intent:Intent,pub simulate_media:bool,pub bpc:bool,pub assume_untagged:bool}
fn proof_pixels(input:&Input,monitor:&Profile,printer:&Profile,indices:&[usize],options:ProofOptions)->R<Vec<u8>>{
 let proof_intent=if options.simulate_media{Intent::AbsoluteColorimetric}else{Intent::RelativeColorimetric};
 let flags=if options.bpc&&!options.simulate_media{Flags::SOFTPROOFING|Flags::BLACKPOINT_COMPENSATION}else{Flags::SOFTPROOFING};
 let mut out=vec![[0u8;3];indices.len()];
 match &input.pixels{
  Pixels::Rgb(p)=>{
   let t:Transform<[u16;3],[u8;3]>=Transform::new_proofing(&input.profile,PixelFormat::RGB_16,monitor,PixelFormat::RGB_8,printer,options.intent,proof_intent,flags).map_err(err)?;
   let v:Vec<_>=indices.iter().map(|&i|p[i]).collect();t.transform_pixels(&v,&mut out);
  }
  Pixels::Cmyk(p)=>{
   let t:Transform<[u16;4],[u8;3]>=Transform::new_proofing(&input.profile,PixelFormat::CMYK_16,monitor,PixelFormat::RGB_8,printer,options.intent,proof_intent,flags).map_err(err)?;
   let v:Vec<_>=indices.iter().map(|&i|p[i]).collect();t.transform_pixels(&v,&mut out);
  }
 }
 Ok(out.into_iter().flatten().collect())
}
fn tagged_png(path:&Path,w:u32,h:u32,rgb:&[u8],icc:&[u8])->R<()>{
 use image::ImageEncoder;
 let mut encoder=image::codecs::png::PngEncoder::new(BufWriter::new(File::create(path).map_err(err)?));
 encoder.set_icc_profile(icc.to_vec()).map_err(err)?;
 encoder.write_image(rgb,w,h,image::ExtendedColorType::Rgb8).map_err(err)
}
pub fn export_proof(source:&Path,reference:&Path,candidate:&Path,display:&Path,root:&Path,options:ProofOptions)->R<PathBuf>{
 let a=crate::comparison::open_printer(reference)?;let b=crate::comparison::open_printer(candidate)?;
 if fs::metadata(display).map_err(err)?.len()>128*1024*1024{return Err("Monitor ICC exceeds 128 MiB".into())}
 let display_data=fs::read(display).map_err(err)?;
 let monitor=Profile::new_icc(&display_data).map_err(err)?;
 if monitor.color_space()!=ColorSpaceSignature::RgbData||monitor.device_class()!=lcms2::ProfileClassSignature::DisplayClass{return Err("Choose an RGB display/monitor ICC profile, not a printer or DeviceLink profile".into())}
 let input=load(source,&a,options.assume_untagged)?;
 let (tw,th,idx)=thumb_indices(input.width,input.height);
 let stage=tempfile::tempdir_in(root).map_err(err)?;
 for (name,printer) in [("before_monitor.png",&a),("after_monitor.png",&b)]{
  let pixels=proof_pixels(&input,&monitor,printer,&idx,options)?;
  tagged_png(&stage.path().join(name),tw,th,&pixels,&display_data)?;
 }
 fs::write(stage.path().join("proof_settings.txt"),format!("Source: {}\nReference: {}\nCandidate: {}\nMonitor: {}\nIntent: {:?}\nSimulate media: {}\nBlack point compensation requested: {} (disabled for absolute media simulation)\n{} x {} preview\nOpen in a color-managed application on the calibrated monitor. PNG contains the chosen monitor ICC. Do not assign sRGB or use this file as print separations. Predicted appearance, not a physical measurement.",source.display(),reference.display(),candidate.display(),display.display(),options.intent,options.simulate_media,options.bpc,tw,th)).map_err(err)?;
 let name=source.file_stem().ok_or("Missing filename")?.to_string_lossy();
 let dest=root.join(format!("{name}_SoftProof"));
 if dest.exists(){return Err("Soft proof directory already exists".into())}
 fs::rename(stage.path(),&dest).map_err(err)?;Ok(dest)
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
 #[test]fn monitor_proof_embeds_display_profile(){
 let d=tempfile::tempdir().unwrap();let a=d.path().join("a.icc");let b=d.path().join("b.icc");let monitor=d.path().join("display.icc");let source=d.path().join("input.png");
 crate::synthetic_cmyk_v4(false).save_new(&a).unwrap();crate::synthetic_cmyk_v4(true).save_new(&b).unwrap();
 let data=Profile::new_srgb().icc().unwrap();std::fs::write(&monitor,&data).unwrap();
 tagged_png(&source,2,1,&[80,100,120,120,180,210],&data).unwrap();
 let out=export_proof(&source,&a,&b,&monitor,d.path(),ProofOptions{intent:Intent::RelativeColorimetric,simulate_media:false,bpc:false,assume_untagged:false}).unwrap();
 let mut decoder=image::ImageReader::open(out.join("after_monitor.png")).unwrap().into_decoder().unwrap();
 assert_eq!(decoder.dimensions(),(2,1));assert_eq!(decoder.icc_profile().unwrap().unwrap(),data);
 }
 #[test]fn thumbnails(){let (w,h,i)=thumb_indices(1280,640);assert_eq!((w,h,i.len()),(640,320,204800));assert!(i.iter().all(|&v|v<1280*640));}
 #[test]fn rejects_huge(){assert!(check_size(10000,10000).is_err());}
 #[test]fn cmyk_tiff_roundtrip(){let d=tempfile::tempdir().unwrap();let p=d.path().join("a.tif");let input=[[0,100,500,65535],[65535,0,0,0]];write_cmyk_tiff(&p,2,1,&input,b"test tag",300).unwrap();let mut r=Decoder::new(File::open(p).unwrap()).unwrap();assert_eq!(r.colortype().unwrap(),ColorType::CMYK(16));assert_eq!(r.get_tag_u8_vec(Tag::IccProfile).unwrap(),b"test tag");match r.read_image().unwrap(){DecodingResult::U16(v)=>assert_eq!(v,input.into_iter().flatten().collect::<Vec<_>>()),_=>panic!("wrong depth")}}
}
