//! Conservative edits to supported four-channel ICC LUTs.
//! LCMS validates opening and transform creation; explicit LUT permutation is
//! necessary to preserve the exact channel indexing. No measured recalibration.
use super::{Profile,Lut,R,put16,put32,interp,u32at};
use std::path::Path;

fn word(x:f64)->u16{(x.clamp(0.,1.)*65535.).round() as u16}
fn curves(t:&mut Vec<u8>,set:&[Vec<f64>]){
 for c in set{t.extend(b"curv\0\0\0\0");put32(t,c.len() as u32);for &v in c{put16(t,word(v));}while t.len()%4!=0{t.push(0)}}
}
fn encode(l:&Lut,kind:&[u8;4])->R<Vec<u8>>{
 let mut t=Vec::new();
 if kind==b"mft2"{
  let ni=l.in_curves[0].len();let no=l.out_curves[0].len();
  if ni>65535||no>65535||l.in_curves.iter().any(|c|c.len()!=ni)||l.out_curves.iter().any(|c|c.len()!=no){return Err("Unequal or oversized legacy curve tables".into())}
  t.extend(b"mft2\0\0\0\0");t.extend([l.inputs as u8,l.outputs as u8,l.grid as u8,0]);
  for i in 0..9{put32(&mut t,if i%4==0{65536}else{0})}
  put16(&mut t,ni as u16);put16(&mut t,no as u16);
  for c in &l.in_curves{for &v in c{put16(&mut t,word(v))}}
  for &v in &l.clut{put16(&mut t,word(v))}
  for c in &l.out_curves{for &v in c{put16(&mut t,word(v))}}
 }else if kind==b"mAB "||kind==b"mBA "{
  let mut b=Vec::new();let mut a=Vec::new();
  if kind==b"mAB "{curves(&mut b,&l.out_curves);curves(&mut a,&l.in_curves)}
  else{curves(&mut b,&l.in_curves);curves(&mut a,&l.out_curves)}
  let mut cl=Vec::new();cl.extend(vec![l.grid as u8;l.inputs]);cl.resize(16,0);cl.extend([2,0,0,0]);for &v in &l.clut{put16(&mut cl,word(v))}while cl.len()%4!=0{cl.push(0)}
  t.extend(kind);t.extend([0;4]);t.extend([l.inputs as u8,l.outputs as u8,0,0]);
  put32(&mut t,32);put32(&mut t,0);put32(&mut t,0);put32(&mut t,(32+b.len()) as u32);put32(&mut t,(32+b.len()+cl.len()) as u32);
  t.extend(b);t.extend(cl);t.extend(a);
 }else{return Err("Editing supports mft2 and simple mAB/mBA only".into())}
 Ok(t)
}
fn checked_lut(t:&[u8],forward:bool)->R<(Lut,[u8;4])>{
 let kind:[u8;4]=t.get(..4).ok_or("Truncated LUT")?.try_into().unwrap();
 if &kind!=b"mft2"&& &kind!=if forward{b"mAB "}else{b"mBA "}{return Err("Unsupported LUT type for editing".into())}
 let l=Lut::parse(t)?;
 if (l.inputs,l.outputs)!=if forward{(4,3)}else{(3,4)}{return Err("Unexpected LUT channels".into())}
 Ok((l,kind))
}
fn valid_order(order:[usize;4])->bool{let mut seen=[false;4];for i in order{if i>=4||seen[i]{return false}seen[i]=true;}true}
fn permute_forward(l:&mut Lut,order:[usize;4]){
 let original=l.clut.clone();let g=l.grid;
 l.in_curves=order.iter().map(|&i|l.in_curves[i].clone()).collect();
 for index in 0..g.pow(4){
  let mut q=index;let mut new=[0;4];for j in (0..4).rev(){new[j]=q%g;q/=g}
  let mut old=[0;4];for j in 0..4{old[order[j]]=new[j]}
  let src=old.iter().fold(0,|acc,&v|acc*g+v);
  l.clut[index*3..index*3+3].copy_from_slice(&original[src*3..src*3+3]);
 }
}
fn permute_reverse(l:&mut Lut,order:[usize;4]){
 l.out_curves=order.iter().map(|&i|l.out_curves[i].clone()).collect();
 for p in l.clut.chunks_exact_mut(4){let old=[p[0],p[1],p[2],p[3]];for j in 0..4{p[j]=old[order[j]]}}
}
fn supported_tags(p:&Profile)->R<()>{
 // Fail closed for alternate transforms, named colors, calibration and vendor
 // metadata whose channel order is not understood. Do not silently drop them.
 const ALLOWED:&[[u8;4]]=&[*b"A2B0",*b"A2B1",*b"A2B2",*b"B2A0",*b"B2A1",*b"B2A2",*b"desc",*b"cprt",*b"wtpt",*b"bkpt",*b"chad",*b"dmnd",*b"dmdd",*b"tech",*b"vued",*b"view",*b"clrt",*b"clro",*b"gamt",*b"pre0",*b"pre1",*b"pre2"];
 for k in p.tags.keys(){if !ALLOWED.contains(k){return Err(format!("Channel reorder blocked by unhandled tag {}",String::from_utf8_lossy(k)))}}
 Ok(())
}
impl Profile{
 /// Compose a gamma transfer curve after the B2A output curves.
 /// A2B is retained as the physical printer model. This is a deliberate
 /// separation edit, not a new measurement-based colorimetric profile.
 pub fn tone_gamma(&self,gamma:[f64;4])->R<Self>{
  self.check()?;
  if gamma.iter().any(|v|!v.is_finite()||!(0.25..=4.).contains(v)){return Err("Gamma must be 0.25–4.0".into())}
  let mut out=self.clone();let mut count=0;
  for key in [b"B2A0",b"B2A1",b"B2A2"]{
   let Some(t)=self.tags.get(key)else{continue};
   let (mut lut,kind)=checked_lut(t,false)?;
   // 4096 samples per output curve; original curves are sampled first.
   for j in 0..4{let old=&lut.out_curves[j];lut.out_curves[j]=(0..4096).map(|i|interp(old,i as f64/4095.).powf(gamma[j])).collect();}
   out.tags.insert(*key,encode(&lut,&kind)?);count+=1;
  }
  if count==0||!out.tags.contains_key(b"B2A0"){return Err("Missing B2A0 output transform".into())}
  out.clean_stale();Ok(out)
 }
 /// order[new_index] = old_index. Both transform directions are permuted.
 /// Only known channel-independent metadata and supported colorant tables
 /// are accepted; arbitrary vendor tags require an explicit adapter.
 pub fn reorder_channels(&self,order:[usize;4])->R<Self>{
  self.check()?;if !valid_order(order){return Err("Mapping must be a permutation of all four channels".into())}
  supported_tags(self)?;
  let mut out=self.clone();let mut forward=0;let mut reverse=0;
  for key in [b"A2B0",b"A2B1",b"A2B2",b"B2A0",b"B2A1",b"B2A2"]{
   let Some(t)=self.tags.get(key)else{continue};
   let f=key[0]==b'A';let (mut lut,kind)=checked_lut(t,f)?;
   if f{permute_forward(&mut lut,order);forward+=1}else{permute_reverse(&mut lut,order);reverse+=1}
   out.tags.insert(*key,encode(&lut,&kind)?);
  }
  if forward==0||reverse==0||!out.tags.contains_key(b"A2B0")||!out.tags.contains_key(b"B2A0"){return Err("Both A2B0 and B2A0 are required".into())}
  if let Some(t)=self.tags.get(b"clrt"){
   if !t.starts_with(b"clrt")||u32at(t,8)?!=4||t.len()<12+4*38{return Err("Unsupported colorant table".into())}
   let mut table=t.clone();for j in 0..4{table[12+j*38..12+(j+1)*38].copy_from_slice(&t[12+order[j]*38..12+(order[j]+1)*38]);}
   out.tags.insert(*b"clrt",table);
  }
  if self.tags.contains_key(b"clro"){return Err("Colorant order tag requires a dedicated mapping adapter".into())}
  out.clean_stale();Ok(out)
 }
}
pub enum Edit{Tone([f64;4]),Reorder([usize;4]),Rename(String)}
pub fn edit_file(source:&Path,destination:&Path,edit:Edit)->R<String>{
 let p=Profile::read(source)?;
 let (mut out,name)=match edit{
  Edit::Tone(g)=>(p.tone_gamma(g)?,None),
  Edit::Reorder(o)=>(p.reorder_channels(o)?,None),
  Edit::Rename(n)=>{if n.trim().is_empty()||n.len()>240{return Err("Profile name must contain 1–240 UTF-8 bytes".into())}(p,Some(n))}
 };
 let description=name.unwrap_or_else(||destination.file_stem().unwrap_or_default().to_string_lossy().into_owned());
 out.describe(&description);
 let bytes=out.bytes()?;
 let parsed=lcms2::Profile::new_icc(&bytes).map_err(|e|format!("LittleCMS rejected edited ICC: {e}"))?;
 if parsed.color_space()==lcms2::ColorSpaceSignature::CmykData{
  let srgb=lcms2::Profile::new_srgb();
  for intent in [lcms2::Intent::Perceptual,lcms2::Intent::RelativeColorimetric,lcms2::Intent::Saturation,lcms2::Intent::AbsoluteColorimetric]{
   let _:lcms2::Transform<[u8;3],[u16;4]>=lcms2::Transform::new(&srgb,lcms2::PixelFormat::RGB_8,&parsed,lcms2::PixelFormat::CMYK_16,intent).map_err(|e|format!("Edited output transform: {e}"))?;
   let _:lcms2::Transform<[u16;4],[u8;3]>=lcms2::Transform::new(&parsed,lcms2::PixelFormat::CMYK_16,&srgb,lcms2::PixelFormat::RGB_8,intent).map_err(|e|format!("Edited forward transform: {e}"))?;
  }
 }
 out.save_new(destination)?;
 Ok(format!("Saved edited ICC: {}",destination.display()))
}
#[cfg(test)]
mod tests{
 use super::*;
 fn lut4()->Lut{let mut v=Vec::new();for i in 0..16{v.extend([(i/8) as f64,((i/4)%2) as f64,(i%2) as f64]);}Lut{inputs:4,outputs:3,grid:2,in_curves:vec![vec![0.,1.];4],out_curves:vec![vec![0.,1.];3],clut:v}}
 #[test]fn forward_axis_permutation(){let mut l=lut4();let old=l.eval4([0.1,0.2,0.3,0.4]);permute_forward(&mut l,[2,1,0,3]);let new=l.eval4([0.3,0.2,0.1,0.4]);for i in 0..3{assert!((old[i]-new[i]).abs()<1e-12);}}
 #[test]fn v4_round_trip(){let l=lut4();let t=encode(&l,b"mAB ").unwrap();let v=Lut::parse(&t).unwrap();assert_eq!(l.clut,v.clut);assert_eq!(l.in_curves,v.in_curves);}
 #[test]fn reverse_channels(){let mut l=Lut{inputs:3,outputs:4,grid:2,in_curves:vec![vec![0.,1.];3],out_curves:vec![vec![0.,1.];4],clut:[0.1,0.2,0.3,0.4].repeat(8)};permute_reverse(&mut l,[2,1,0,3]);assert_eq!(&l.clut[..4],&[0.3,0.2,0.1,0.4]);}
 #[test]fn rejects_duplicate_channels(){assert!(!valid_order([0,0,2,3]));assert!(valid_order([2,1,0,3]));}
}
