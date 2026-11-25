//! TIFF writer exclusively for generated ICC characterization charts.
use std::{fs::File,io::BufWriter,path::Path};
use tiff::{encoder::{TiffEncoder,colortype::CMYK16,Rational},tags::{Tag,ResolutionUnit}};
type R<T>=Result<T,String>;
fn err(e:impl std::fmt::Display)->String{e.to_string()}
fn check_size(w:u32,h:u32)->R<usize>{
 let n=w as u64*h as u64;if n==0||n>24_000_000{return Err("Preview converter supports up to 24 megapixels; larger jobs require a tiled/streaming implementation.".into())}Ok(n as usize)
}
pub fn write_cmyk_tiff(path:&Path,w:u32,h:u32,pixels:&[[u16;4]],icc:&[u8],dpi:u32)->R<()>{
 if pixels.len()!=check_size(w,h)?||!(50..=2400).contains(&dpi){return Err("Invalid TIFF size or DPI".into())}
 let mut e=TiffEncoder::new(BufWriter::new(File::create(path).map_err(err)?)).map_err(err)?;
 let mut img=e.new_image::<CMYK16>(w,h).map_err(err)?;
 img.resolution(ResolutionUnit::Inch,Rational{n:dpi,d:1});
 if !icc.is_empty(){img.encoder().write_tag(Tag::IccProfile,icc).map_err(err)?;}
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
#[cfg(test)]mod tests{
 use super::*;
 use tiff::{decoder::{Decoder,DecodingResult},ColorType};
 #[test]fn cmyk_tiff_roundtrip(){let d=tempfile::tempdir().unwrap();let p=d.path().join("a.tif");let input=[[0,100,500,65535],[65535,0,0,0]];write_cmyk_tiff(&p,2,1,&input,b"test tag",300).unwrap();let mut r=Decoder::new(File::open(p).unwrap()).unwrap();assert_eq!(r.colortype().unwrap(),ColorType::CMYK(16));assert_eq!(r.get_tag_u8_vec(Tag::IccProfile).unwrap(),b"test tag");match r.read_image().unwrap(){DecodingResult::U16(v)=>assert_eq!(v,input.into_iter().flatten().collect::<Vec<_>>()),_=>panic!("wrong depth")}}
}
