//! CPU raster preview using the existing image crate. No GPU canvas layout
//! dependency. The same Lab scale is used for both profiles and visibility
//! toggles; narrow ceramic gamuts are automatically fitted.
use crate::gamut::{Overlay,Cloud,project};
use image::{Rgb,RgbImage,ImageEncoder};
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct ViewOptions{pub three_d:bool,pub reference:bool,pub candidate:bool,pub slice:bool,pub light:f32,pub yaw:f32,pub pitch:f32,pub zoom:f32,pub wireframe:bool}
impl Default for ViewOptions{fn default()->Self{Self{three_d:true,reference:true,candidate:true,slice:false,light:50.,yaw:0.65,pitch:0.45,zoom:1.,wireframe:false}}}
pub const SIZE:u32=640;
const BEFORE:[u8;3]=[96,165,250];
const AFTER:[u8;3]=[251,191,36];
const BG:[u8;3]=[11,18,32];
#[derive(Debug)]
pub struct Rendered{pub image:RgbImage,pub visible:[usize;2],pub units_per_half:f32}
impl Rendered{
 pub fn png(&self)->Result<Vec<u8>,String>{
  let mut bytes=Vec::new();
  image::codecs::png::PngEncoder::new(&mut bytes).write_image(self.image.as_raw(),SIZE,SIZE,image::ExtendedColorType::Rgb8).map_err(|e|e.to_string())?;
  Ok(bytes)
 }
}
fn projected(v:[f32;3],o:ViewOptions)->[f32;3]{if o.three_d{project(v,o.yaw,o.pitch)}else{[v[1],v[2],0.]}}
fn in_slice(v:[f32;3],o:ViewOptions)->bool{o.three_d||!o.slice||(v[0]-o.light).abs()<=5.}
fn paint(img:&mut RgbImage,x:i32,y:i32,color:[u8;3],alpha:u16){
 if x<0||y<0||x>=SIZE as i32||y>=SIZE as i32{return}
 let p=img.get_pixel_mut(x as u32,y as u32);
 for j in 0..3{p.0[j]=((p.0[j] as u16*(255-alpha)+color[j] as u16*alpha)/255) as u8;}
}
fn dot(img:&mut RgbImage,p:[f32;2],color:[u8;3],radius:i32,alpha:u16){
 let x=p[0].round() as i32;let y=p[1].round() as i32;
 for i in -radius..=radius{for j in -radius..=radius{if i*i+j*j<=radius*radius{paint(img,x+i,y+j,color,alpha);}}}
}
fn line(img:&mut RgbImage,a:[f32;2],b:[f32;2],color:[u8;3],radius:i32,alpha:u16){
 let n=((a[0]-b[0]).abs().max((a[1]-b[1]).abs()).ceil() as usize).clamp(1,4096);
 for i in 0..=n{let t=i as f32/n as f32;dot(img,[a[0]+(b[0]-a[0])*t,a[1]+(b[1]-a[1])*t],color,radius,alpha);}
}
fn outline(mut p:Vec<[f32;2]>)->Vec<[f32;2]>{
 p.sort_by(|a,b|a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));p.dedup();
 if p.len()<3{return p}
 fn cross(a:[f32;2],b:[f32;2],c:[f32;2])->f32{(b[0]-a[0])*(c[1]-a[1])-(b[1]-a[1])*(c[0]-a[0])}
 let half=|points:&[[f32;2]]|{let mut h=Vec::new();for &v in points{while h.len()>1&&cross(h[h.len()-2],h[h.len()-1],v)<=0.{h.pop();}h.push(v);}h};
 let mut lo=half(&p);p.reverse();let mut hi=half(&p);lo.pop();hi.pop();lo.extend(hi);lo
}
pub fn render(data:Option<&Overlay>,o:ViewOptions)->Rendered{
 let mut img=RgbImage::from_pixel(SIZE,SIZE,Rgb(BG));
 let clouds:[Option<&Cloud>;2]=[data.and_then(|d|d.reference.as_ref().ok()),data.and_then(|d|d.candidate.as_ref()).and_then(|d|d.as_ref().ok())];
 // Fit all points (not only visible ones) in a rotation-invariant frame.
 // This avoids rescaling when the user toggles Before/After or rotates.
 let extent=clouds.iter().flatten().flat_map(|c|c.points.iter()).filter(|v|v.iter().all(|x|x.is_finite())).map(|v|if o.three_d{((v[0]-50.).powi(2)+v[1].powi(2)+v[2].powi(2)).sqrt()}else{v[1].abs().max(v[2].abs())}).fold(20f32,f32::max);
 let range=(extent*1.18/10.).ceil()*10.;
 let zoom=if o.zoom.is_finite(){o.zoom.clamp(0.5,3.)}else{1.};
 let scale=(SIZE as f32/2.-28.)/range*zoom;
 let at=|v:[f32;3]|[SIZE as f32/2.+v[0]*scale,SIZE as f32/2.-v[1]*scale];
 let p=|v:[f32;3]|at(projected(v,o));
 let step=if range>100.{20.}else if range>50.{10.}else{5.};
 let tick_count=(range/step).floor() as i32;
 // The axis/grid is always rendered, including before a profile is opened.
 if !o.three_d{
  for i in -tick_count..=tick_count{
   let v=i as f32*step;
   line(&mut img,p([50.,v,-range]),p([50.,v,range]),[40,54,74],0,255);
   line(&mut img,p([50.,-range,v]),p([50.,range,v]),[40,54,74],0,255);
  }
 }
 let axes=[([50.,-range,0.],[50.,range,0.],[232,121,121]),([50.,0.,-range],[50.,0.,range],[102,187,138]),([0.,0.,0.],[100.,0.,0.],[229,231,235])];
 for (i,(a,b,c)) in axes.into_iter().enumerate(){
  if i==2&&!o.three_d{continue}
  line(&mut img,p(a),p(b),c,1,215);
  for t in 0..=10{let f=t as f32/10.;let v=std::array::from_fn(|j|a[j]+(b[j]-a[j])*f);dot(&mut img,p(v),c,3,220);}
  dot(&mut img,p(b),c,5,255);
 }
 let mut dots=Vec::new();let mut visible=[0;2];
 for (i,cloud) in clouds.into_iter().enumerate(){
  if !(if i==0{o.reference}else{o.candidate}){continue}
  let Some(c)=cloud else{continue};let color=if i==0{BEFORE}else{AFTER};
  let points:Vec<_>=c.points.iter().copied().filter(|v|v.iter().all(|v|v.is_finite())&&in_slice(*v,o)).collect();
  visible[i]=points.len();
  if o.wireframe&&o.three_d{
   if let Some(h)=&c.hull{
    for &[a,b] in &h.edges{if let (Some(&a),Some(&b))=(h.vertices.get(a),h.vertices.get(b)){line(&mut img,p(a),p(b),color,1,210);}}
    continue
   }
  }
  if o.wireframe&&!o.three_d&&points.len()>=3{
   let h=outline(points.iter().map(|v|[v[1],v[2]]).collect());
   if h.len()>=3{for j in 0..h.len(){let a=h[j];let b=h[(j+1)%h.len()];line(&mut img,at([a[0],a[1],0.]),at([b[0],b[1],0.]),color,2,235);}continue}
  }
  for v in points{dots.push((projected(v,o),color));}
 }
 if o.three_d{dots.sort_by(|a,b|a.0[2].total_cmp(&b.0[2]));}
 for (v,c) in dots{dot(&mut img,at(v),c,3,210);}
 Rendered{image:img,visible,units_per_half:range/zoom}
}
/// A second presentation backend uses ordinary GPUI rectangle elements,
/// independent of canvas hooks, texture atlases and image decoding.
pub const CELL_GRID:u32=160;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Span{pub x:u32,pub y:u32,pub width:u32,pub rgb:u32}
pub fn native_spans(image:&RgbImage)->Vec<Span>{
 let mut spans=Vec::new();
 let factor=SIZE/CELL_GRID;
 let color_at=|x:u32,y:u32|{
  let mut chosen=BG;let mut best=0u32;
  for dy in 0..factor{for dx in 0..factor{
   let p=image.get_pixel(x*factor+dx,y*factor+dy).0;
   let score=p.iter().zip(BG).map(|(&a,b)|u32::from(a.abs_diff(b))).sum::<u32>();
   if score>best{best=score;chosen=p;}
  }}
  if best==0{0}else{u32::from(chosen[0])<<16|u32::from(chosen[1])<<8|u32::from(chosen[2])}
 };
 for y in 0..CELL_GRID{
  let mut x=0;
  while x<CELL_GRID{
   let rgb=color_at(x,y);let start=x;x+=1;
   while x<CELL_GRID&&color_at(x,y)==rgb{x+=1;}
   if rgb!=0{spans.push(Span{x:start,y,width:x-start,rgb});}
  }
 }
 spans
}
#[cfg(test)]mod tests{
 use super::*;
 fn example()->Overlay{
  let points=vec![[30.,-4.,-40.],[50.,12.,20.],[98.,0.,0.],[70.,22.,-25.]];
  Overlay{reference:Ok(Cloud{points:points.clone(),sampled:4,errors:vec![0.;4],hull:None}),candidate:Some(Ok(Cloud{points,sampled:4,errors:vec![0.;4],hull:None}))}
 }
 #[test]fn native_spans_include_grid_even_without_icc(){
  for three_d in [false,true]{
   let r=render(None,ViewOptions{three_d,..Default::default()});
   let spans=native_spans(&r.image);
   assert!(!spans.is_empty());assert!(spans.iter().all(|s|s.x+s.width<=CELL_GRID&&s.y<CELL_GRID));
   assert!(spans.iter().any(|s|s.rgb>>16>100));
  }
 }
 #[test]fn native_spans_change_with_profile(){
  let d=example();let o=ViewOptions{three_d:false,candidate:false,..Default::default()};
  let a=native_spans(&render(Some(&d),o).image);
  let b=native_spans(&render(Some(&d),ViewOptions{reference:false,..o}).image);
  assert_ne!(a,b);
 }
 #[test]fn actual_pixels_change_in_both_views(){
  let d=example();
  for three_d in [false,true]{
   let o=ViewOptions{three_d,candidate:false,..Default::default()};
   let r=render(Some(&d),o);let hidden=render(Some(&d),ViewOptions{reference:false,..o});
   assert_eq!(r.visible,[4,0]);assert_eq!(hidden.visible,[0,0]);assert_eq!(r.units_per_half,hidden.units_per_half);
   assert_ne!(r.image.as_raw(),hidden.image.as_raw());
   assert!(r.image.pixels().any(|p|p[2]>160&&p[2]>p[0]));
   assert_eq!(&r.png().unwrap()[..8],b"\x89PNG\r\n\x1a\n");
  }
 }
 #[test]fn slice_and_toggle(){
  let d=example();let r=render(Some(&d),ViewOptions{three_d:false,slice:true,light:50.,reference:false,..Default::default()});
  assert_eq!(r.visible,[0,1]);
 }
 #[test]fn native_icc_renders_nonempty(){
  let d=tempfile::tempdir().unwrap();let p=d.path().join("native.icc");crate::synthetic_cmyk_v4(true).save_new(&p).unwrap();
  let data=crate::gamut::overlay(&p,None,9);assert!(data.reference.as_ref().unwrap().points.len()>5);
  for three_d in [false,true]{let r=render(Some(&data),ViewOptions{three_d,..Default::default()});assert!(r.visible[0]>5);assert!(r.image.pixels().any(|p|p[2]>160&&p[2]>p[0]));}
 }
}
