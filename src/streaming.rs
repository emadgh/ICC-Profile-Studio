//! Strip-based RGB/CMYK TIFF conversion with bounded working memory.
use std::{fs::{self,File},io::BufReader,path::Path,sync::atomic::{AtomicBool,Ordering}};
use lcms2::{Profile,Transform,PixelFormat,ColorSpaceSignature};
use tiff::{ColorType,decoder::{Decoder,DecodingResult},encoder::{TiffEncoder,TiffKind,TiffKindBig,TiffKindStandard,colortype::{CMYK16,Gray16},Rational},tags::{Tag,ResolutionUnit}};
use image::{ImageBuffer,Rgb,Luma};
use crate::images::{Options,Output,thumb_indices,proof};
type R<T>=Result<T,String>;
fn err(e:impl std::fmt::Display)->String{e.to_string()}
#[derive(Clone,Copy,Debug,Default)]
pub struct StreamOptions{pub bigtiff:bool,pub separations:bool}
fn cancelled(cancel:&AtomicBool)->R<()>{if cancel.load(Ordering::Relaxed){Err("Cancelled; incomplete output discarded".into())}else{Ok(())}}
fn dimensions(w:u32,h:u32,rows:u32,big:bool)->R<u64>{
 let n=w as u64*h as u64;
 if n==0||n>2_000_000_000||rows==0||w as u64*rows as u64>2_000_000{return Err("Streaming limit: 2 billion pixels total, 2 million pixels per source strip. Repack large strips into smaller ones.".into())}
 if !big&&n*8>3_500_000_000{return Err("Output may exceed classic TIFF limits. Explicitly enable BigTIFF only if the RIP accepts it.".into())}
 Ok(n)
}
enum Conversion{
 Rgb(Transform<[u16;3],[u16;4]>,Transform<[u16;3],[u16;4]>),
 Cmyk(Transform<[u16;4],[u16;4]>,Transform<[u16;4],[u16;4]>),
}
impl Conversion{
 fn new(src:&Profile,a:&Profile,b:&Profile,cmyk:bool,options:Options)->R<Self>{
  if cmyk{
   if src.color_space()!=ColorSpaceSignature::CmykData{return Err("Source ICC must be CMYK".into())}
   Ok(Self::Cmyk(Transform::new(src,PixelFormat::CMYK_16,a,PixelFormat::CMYK_16,options.intent).map_err(err)?,Transform::new(src,PixelFormat::CMYK_16,b,PixelFormat::CMYK_16,options.intent).map_err(err)?))
  }else{
   if src.color_space()!=ColorSpaceSignature::RgbData{return Err("Source ICC must be RGB".into())}
   Ok(Self::Rgb(Transform::new(src,PixelFormat::RGB_16,a,PixelFormat::CMYK_16,options.intent).map_err(err)?,Transform::new(src,PixelFormat::RGB_16,b,PixelFormat::CMYK_16,options.intent).map_err(err)?))
  }
 }
 fn strip(&self,raw:&[u16],n:usize,local:&[usize])->R<(Vec<[u16;4]>,Vec<[u16;4]>)>{
  let mut after=vec![[0u16;4];n];let mut before=vec![[0u16;4];local.len()];
  match self{
   Self::Rgb(a,b)=>{
    if raw.len()!=n*3{return Err("RGB strip length mismatch".into())}
    let values:Vec<_>=raw.chunks_exact(3).map(|v|[v[0],v[1],v[2]]).collect();
    b.transform_pixels(&values,&mut after);let subset:Vec<_>=local.iter().map(|&i|values[i]).collect();
    if !subset.is_empty(){a.transform_pixels(&subset,&mut before);}
   }
   Self::Cmyk(a,b)=>{
    if raw.len()!=n*4{return Err("CMYK strip length mismatch".into())}
    let values:Vec<_>=raw.chunks_exact(4).map(|v|[v[0],v[1],v[2],v[3]]).collect();
    b.transform_pixels(&values,&mut after);let subset:Vec<_>=local.iter().map(|&i|values[i]).collect();
    if !subset.is_empty(){a.transform_pixels(&subset,&mut before);}
   }
  }
  Ok((after,before))
 }
}
#[derive(Default)]
struct Stats{sum:[f64;4],max:[f64;4],peak:f64,over:usize}
impl Stats{
 fn add(&mut self,p:&[[u16;4]],cap:f64){
  for v in p{
   let v=v.map(|x|x as f64*100./65535.);let total=v.iter().sum::<f64>();self.peak=self.peak.max(total);
   if total>cap+1e-8{self.over+=1}
   for j in 0..4{self.sum[j]+=v[j];self.max[j]=self.max[j].max(v[j]);}
  }
 }
}
pub fn convert(source:&Path,reference:&Path,destination:&Path,root:&Path,options:Options,stream:StreamOptions,cancel:&AtomicBool,progress:impl Fn(u32,u32))->R<Output>{
 cancelled(cancel)?;
 if !(50..=2400).contains(&options.dpi)||!options.tac.is_finite()||!(0.0..=400.).contains(&options.tac){return Err("Invalid conversion settings".into())}
 let a=crate::comparison::open_printer(reference)?;let b=crate::comparison::open_printer(destination)?;
 let mut decoder=Decoder::new(BufReader::new(File::open(source).map_err(err)?)).map_err(err)?;
 if decoder.more_images()||decoder.get_tag_unsigned::<u16>(Tag::Orientation).unwrap_or(1)!=1||decoder.get_tag_unsigned::<u16>(Tag::PlanarConfiguration).unwrap_or(1)!=1{return Err("Streaming requires single-image, top-left, interleaved TIFF".into())}
 let color=decoder.colortype().map_err(err)?;
 let cmyk=matches!(color,ColorType::CMYK(8)|ColorType::CMYK(16));
 if !cmyk&&!matches!(color,ColorType::RGB(8)|ColorType::RGB(16)){return Err("Streaming requires RGB or CMYK, 8/16 bit, no alpha".into())}
 let count=decoder.strip_count().map_err(|_|"Streaming requires strips, not tiles".to_owned())?;
 let (w,h)=decoder.dimensions().map_err(err)?;let (cw,rows)=decoder.chunk_dimensions();
 if cw!=w||rows==0||count!=h.div_ceil(rows){return Err("Unsupported strip geometry".into())}
 let n=dimensions(w,h,rows,stream.bigtiff)?;
 let src=match decoder.get_tag_u8_vec(Tag::IccProfile){
  Ok(bytes)=>Profile::new_icc(&bytes).map_err(err)?,
  Err(_) if options.assume_untagged&&cmyk=>Profile::new_icc(&a.icc().map_err(err)?).map_err(err)?,
  Err(_) if options.assume_untagged=>Profile::new_srgb(),
  Err(_)=>return Err("Untagged image: explicitly select the source assumption".into())
 };
 let conversion=Conversion::new(&src,&a,&b,cmyk,options)?;
 let stem=source.file_stem().ok_or("Missing filename")?.to_string_lossy();
 let final_dir=root.join(format!("{stem}_ICC"));
 if final_dir.exists(){return Err("Output directory already exists".into())}
 let stage=tempfile::tempdir_in(root).map_err(err)?;
 let profile=b.icc().map_err(err)?;
 let (tw,th,indices)=thumb_indices(w,h);
 let output=stage.path().join("converted.tif");
 let (stats,before,after)=if stream.bigtiff{
  write_strips::<TiffKindBig>(&mut decoder,&conversion,&output,&profile,w,h,rows,count,&indices,options,stream,cancel,&progress)?
 }else{
  write_strips::<TiffKindStandard>(&mut decoder,&conversion,&output,&profile,w,h,rows,count,&indices,options,stream,cancel,&progress)?
 };
 cancelled(cancel)?;
 let thumbnail_indices:Vec<_>=(0..indices.len()).collect();
 proof(&before,&a,&thumbnail_indices,tw,th,&stage.path().join("before.png"))?;
 proof(&after,&b,&thumbnail_indices,tw,th,&stage.path().join("after.png"))?;
 for j in 0..4{
  let v=after.iter().map(|p|255-(p[j] as u32*255/65535) as u8).collect::<Vec<_>>();
  ImageBuffer::<Luma<u8>,_>::from_raw(tw,th,v).ok_or("Invalid separation thumbnail")?.save(stage.path().join(format!("channel_{}.png",j+1))).map_err(err)?;
 }
 let heat:Vec<u8>=after.iter().flat_map(|p|{let t=p.iter().map(|&v|v as f64/65535.).sum::<f64>()/4.;[(255.*t) as u8,(255.*(1.-(2.*t-1.).abs())) as u8,(255.*(1.-t)) as u8]}).collect();
 ImageBuffer::<Rgb<u8>,_>::from_raw(tw,th,heat).ok_or("Invalid coverage thumbnail")?.save(stage.path().join("coverage.png")).map_err(err)?;
 let mean=stats.sum.map(|v|v/n as f64);
 fs::write(stage.path().join("report.txt"),format!("Strip conversion: {w} × {h}, {} DPI, CMYK16, BigTIFF {}\nSource: {}\nDestination: {}\nMean inks {:?}\nMax inks {:?}\nPeak TAC {:.4}%\nFull-size separation TIFFs: {} (white=no ink, black=max ink, not ICC-tagged print data).\nPreview uses media-relative sRGB, not measured or calibrated proof. Source DPI/metadata not copied.",options.dpi,stream.bigtiff,source.display(),destination.display(),mean,stats.max,stats.peak,stream.separations)).map_err(err)?;
 cancelled(cancel)?;fs::rename(stage.path(),&final_dir).map_err(err)?;
 Ok(Output{directory:final_dir,width:w,height:h,mean,max:stats.max,peak_tac:stats.peak,over_tac:stats.over})
}
#[allow(clippy::too_many_arguments)]
fn write_strips<K:TiffKind>(decoder:&mut Decoder<BufReader<File>>,conversion:&Conversion,path:&Path,icc:&[u8],w:u32,h:u32,rows:u32,count:u32,indices:&[usize],options:Options,stream:StreamOptions,cancel:&AtomicBool,progress:&impl Fn(u32,u32))->R<(Stats,Vec<[u16;4]>,Vec<[u16;4]>)>{
 let mut main:TiffEncoder<File,K>=TiffEncoder::new_generic(File::create(path).map_err(err)?).map_err(err)?;
 let mut img=main.new_image::<CMYK16>(w,h).map_err(err)?;
 img.resolution(ResolutionUnit::Inch,Rational{n:options.dpi,d:1});img.rows_per_strip(rows).map_err(err)?;
 img.encoder().write_tag(Tag::IccProfile,icc).map_err(err)?;
 let mut encoders:Vec<TiffEncoder<File,K>>=Vec::new();
 if stream.separations{
  for j in 1..=4{let file=path.with_file_name(format!("separation_{j}_16bit.tif"));encoders.push(TiffEncoder::new_generic(File::create(file).map_err(err)?).map_err(err)?);}
 }
 let mut channels=Vec::new();
 for e in &mut encoders{let mut g=e.new_image::<Gray16>(w,h).map_err(err)?;g.resolution(ResolutionUnit::Inch,Rational{n:options.dpi,d:1});g.rows_per_strip(rows).map_err(err)?;channels.push(g);}
 let mut stats=Stats::default();let mut before=Vec::with_capacity(indices.len());let mut after=Vec::with_capacity(indices.len());
 let mut pos=0usize;let mut thumb=0usize;
 for i in 0..count{
  cancelled(cancel)?;
  let (width,height)=decoder.chunk_data_dimensions(i);
  if width!=w||height!=(h-i*rows).min(rows){return Err("Unexpected TIFF strip dimensions".into())}
  let n=width as usize*height as usize;
  let raw:Vec<u16>=match decoder.read_chunk(i).map_err(err)?{DecodingResult::U8(v)=>v.into_iter().map(|v|v as u16*257).collect(),DecodingResult::U16(v)=>v,_=>return Err("Unsupported strip sample depth".into())};
  let start=thumb;while thumb<indices.len()&&indices[thumb]<pos+n{thumb+=1}
  let local:Vec<_>=indices[start..thumb].iter().map(|&x|x-pos).collect();
  let (p,prior)=conversion.strip(&raw,n,&local)?;
  stats.add(&p,options.tac);
  if stats.over>0{return Err(format!("TAC exceeded: peak {:.4}% > {:.2}%. No output published.",stats.peak,options.tac))}
  before.extend(prior);after.extend(local.iter().map(|&i|p[i]));
  let flat:Vec<u16>=p.iter().flatten().copied().collect();
  if img.next_strip_sample_count()!=flat.len() as u64{return Err("Encoder strip mismatch".into())}
  img.write_strip(&flat).map_err(err)?;
  for (j,g) in channels.iter_mut().enumerate(){let v:Vec<u16>=p.iter().map(|x|65535-x[j]).collect();g.write_strip(&v).map_err(err)?;}
  pos+=n;progress(i+1,count);
 }
 if pos as u64!=w as u64*h as u64||thumb!=indices.len()||img.next_strip_sample_count()!=0{return Err("Incomplete streamed image".into())}
 img.finish().map_err(err)?;
 for g in channels{g.finish().map_err(err)?;}
 Ok((stats,before,after))
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn classic_vs_big_limits(){assert!(dimensions(30000,30000,16,false).is_err());assert!(dimensions(30000,30000,16,true).is_ok());assert!(dimensions(30000,30000,100,true).is_err());}
 #[test]fn cancellation(){let c=AtomicBool::new(true);assert!(cancelled(&c).is_err());}
 #[test]fn stats(){let mut s=Stats::default();s.add(&[[65535,65535,0,0],[0,0,0,0]],199.);assert_eq!(s.peak,200.);assert_eq!(s.over,1);}
}
