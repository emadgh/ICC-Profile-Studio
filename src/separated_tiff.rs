//! Classic TIFF separated unsigned 16-bit samples. TIFF 6.0 section 16.
//! Interleaved physical channels, uncompressed strips, no alpha or ICC tag.
//! For non-CMYK, InkSet=2, NumberOfInks and neutral ordinal InkNames.
//! Reader/RIP support for arbitrary separated TIFF is not universal.
use crate::R;
use std::io::{self,Write};
struct Entry{tag:u16,typ:u16,count:u32,bytes:Vec<u8>}
fn shorts(tag:u16,v:&[u16])->Entry{Entry{tag,typ:3,count:v.len() as u32,bytes:v.iter().flat_map(|x|x.to_le_bytes()).collect()}}
fn longs(tag:u16,v:&[u32])->Entry{Entry{tag,typ:4,count:v.len() as u32,bytes:v.iter().flat_map(|x|x.to_le_bytes()).collect()}}
fn ascii(tag:u16,bytes:Vec<u8>)->Entry{Entry{tag,typ:2,count:bytes.len() as u32,bytes}}
fn rational(tag:u16,v:u32)->Entry{Entry{tag,typ:5,count:1,bytes:[v.to_le_bytes(),1u32.to_le_bytes()].concat()}}
/// Stream scanlines using a caller-supplied recipe renderer. All offsets
/// are checked before writing; at most one scanline is allocated here.
pub(crate) fn write<W:Write>(mut w:W,width:u32,height:u32,n:usize,cmyk:bool,dpi:u32,mut row:impl FnMut(u32,&mut [u16]))->R<()>{
 if !(2..=15).contains(&n)||cmyk&&n!=4||width==0||height==0||!(50..=1200).contains(&dpi)||u64::from(width)*u64::from(height)>24_000_000{return Err("Invalid separated TIFF dimensions, channels or DPI".into())}
 let row_bytes=u64::from(width)*n as u64*2;
 let strip_rows=(1_048_576/row_bytes).clamp(1,32) as u32;
 let strips=height.div_ceil(strip_rows) as usize;
 let counts=(0..strips).map(|i|((height-i as u32*strip_rows).min(strip_rows) as u64*row_bytes) as u32).collect::<Vec<_>>();
 let mut e=vec![
 longs(256,&[width]),longs(257,&[height]),shorts(258,&vec![16;n]),
 shorts(259,&[1]),shorts(262,&[5]),longs(273,&vec![0;strips]),shorts(274,&[1]),
 shorts(277,&[n as u16]),longs(278,&[strip_rows]),longs(279,&counts),
 rational(282,dpi),rational(283,dpi),shorts(284,&[1]),shorts(296,&[2]),
 ascii(305,b"ICC Profile Studio\0".to_vec()),
 shorts(332,&[if cmyk{1}else{2}]),shorts(334,&[n as u16]),shorts(339,&vec![1;n])
 ];
 if !cmyk{
  let names=(1..=n).map(|i|format!("CH{i}\0")).collect::<String>().into_bytes();
  e.push(ascii(333,names));
 }
 e.sort_by_key(|v|v.tag);
 // Header and IFD followed by even-aligned out-of-line tag data.
 let mut cursor=8+2+12*e.len()+4;
 for entry in &e{if entry.bytes.len()>4{cursor=(cursor+1)&!1;cursor+=entry.bytes.len();}}
 cursor=(cursor+1)&!1;
 let data_start=cursor;
 let end=data_start as u64+row_bytes*u64::from(height);
 if end>u32::MAX as u64{return Err("Classic TIFF offset limit exceeded".into())}
 let mut offset=data_start as u32;let mut offsets=Vec::with_capacity(strips);
 for count in &counts{offsets.push(offset);offset=offset.checked_add(*count).ok_or("Strip offset overflow")?;}
 e.iter_mut().find(|v|v.tag==273).unwrap().bytes=offsets.iter().flat_map(|v|v.to_le_bytes()).collect();
 let mut header=b"II\x2a\0\x08\0\0\0".to_vec();
 header.extend_from_slice(&(e.len() as u16).to_le_bytes());
 let mut external=Vec::new();let base=8+2+12*e.len()+4;
 for entry in e{
  header.extend_from_slice(&entry.tag.to_le_bytes());header.extend_from_slice(&entry.typ.to_le_bytes());header.extend_from_slice(&entry.count.to_le_bytes());
  if entry.bytes.len()<=4{let mut inline=[0u8;4];inline[..entry.bytes.len()].copy_from_slice(&entry.bytes);header.extend_from_slice(&inline);}
  else{
   if (base+external.len())%2!=0{external.push(0);}
   header.extend_from_slice(&((base+external.len()) as u32).to_le_bytes());external.extend_from_slice(&entry.bytes);
  }
 }
 header.extend_from_slice(&0u32.to_le_bytes());header.extend(external);header.resize(data_start,0);
 let mut samples=vec![0u16;width as usize*n];let mut bytes=vec![0u8;samples.len()*2];
 let result=(||->io::Result<()>{
  w.write_all(&header)?;
  for y in 0..height{
   samples.fill(0);row(y,&mut samples);
   for (v,dst) in samples.iter().zip(bytes.chunks_exact_mut(2)){dst.copy_from_slice(&v.to_le_bytes());}
   w.write_all(&bytes)?;
  }
  w.flush()
 })();result.map_err(|e|e.to_string())
}
#[cfg(test)]mod tests{
 use super::*;
 fn u16v(b:&[u8],p:usize)->u16{u16::from_le_bytes(b[p..p+2].try_into().unwrap())}
 fn u32v(b:&[u8],p:usize)->u32{u32::from_le_bytes(b[p..p+4].try_into().unwrap())}
 fn tag(b:&[u8],key:u16)->(u32,Vec<u8>){
  let n=u16v(b,8) as usize;
  let p=(0..n).map(|i|10+12*i).find(|&p|u16v(b,p)==key).unwrap();
  let count=u32v(b,p+4);let size=match u16v(b,p+2){2=>1,3=>2,4=>4,5=>8,_=>panic!("bad type")};
  let len=count as usize*size;let start=if len<=4{p+8}else{u32v(b,p+8) as usize};(count,b[start..start+len].to_vec())
 }
 #[test]fn independent_ifd_and_strip_check(){
  for n in [2,3,4,6,8,15]{
   for cmyk in [false,true].into_iter().filter(|v|!*v||n==4){
    let mut b=Vec::new();
    write(&mut b,3,35,n,cmyk,300,|y,r|{for (i,v) in r.iter_mut().enumerate(){*v=if i%n==n-1{0}else{(y*100+i as u32) as u16};}}).unwrap();
    assert_eq!(&b[..4],b"II\x2a\0");assert_eq!(tag(&b,262).1,5u16.to_le_bytes());assert_eq!(tag(&b,277).1,(n as u16).to_le_bytes());
    assert_eq!(tag(&b,258).1,vec![16u16;n].iter().flat_map(|v|v.to_le_bytes()).collect::<Vec<_>>());
    assert_eq!(tag(&b,332).1,(if cmyk{1u16}else{2}).to_le_bytes());assert_eq!(tag(&b,334).1,(n as u16).to_le_bytes());
    if !cmyk{let names=tag(&b,333).1;assert_eq!(names,(1..=n).map(|i|format!("CH{i}\0")).collect::<String>().into_bytes());}
    let offsets=tag(&b,273).1;let counts=tag(&b,279).1;
    let mut y=0u32;
    for (o,c) in offsets.chunks_exact(4).zip(counts.chunks_exact(4)){
     let start=u32v(o,0) as usize;let bytes=u32v(c,0) as usize;
     for r in b[start..start+bytes].chunks_exact(3*n*2){
      for i in 0..3*n{assert_eq!(u16v(r,i*2),if i%n==n-1{0}else{(y*100+i as u32) as u16});}y+=1;
     }
    }assert_eq!(y,35);
    let last=u32v(&offsets,offsets.len()-4) as usize+u32v(&counts,counts.len()-4) as usize;assert_eq!(last,b.len());
   }
  }
 }
 #[test]fn invalid_channel_and_dimensions_refused(){let mut b=Vec::new();assert!(write(&mut b,3,1,16,false,300,|_,_|{}).is_err());assert!(b.is_empty());}
}
