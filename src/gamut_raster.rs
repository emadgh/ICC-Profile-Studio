//! CPU raster preview using the existing image crate. No GPU canvas layout
//! dependency. The same Lab scale is used for both profiles and visibility
//! toggles; narrow ceramic gamuts are automatically fitted.
use crate::gamut::{Overlay,Cloud,project};
use image::{Rgb,RgbImage,ImageEncoder};
#[derive(Clone,Copy,Debug,PartialEq,Eq,serde::Serialize,serde::Deserialize,Default)]
pub enum CompareLayout{#[default] ReferenceSolidCandidateWire,ReferenceWireCandidateSolid,Shared}
#[derive(Clone,Copy,Debug,PartialEq,serde::Serialize,serde::Deserialize)]
pub struct ViewOptions{pub three_d:bool,pub reference:bool,pub candidate:bool,pub slice:bool,pub light:f32,pub yaw:f32,pub pitch:f32,pub zoom:f32,pub wireframe:bool,pub solid:bool,pub profile_colors:bool,pub layout:CompareLayout,pub floor_contour:bool,pub opacity_2d:u8}
impl Default for ViewOptions{fn default()->Self{Self{three_d:true,reference:true,candidate:true,slice:false,light:50.,yaw:0.65,pitch:0.45,zoom:1.,wireframe:false,solid:true,profile_colors:true,layout:CompareLayout::default(),floor_contour:true,opacity_2d:100}}}
impl ViewOptions{
 pub fn style(self,i:usize)->(bool,bool){
  match self.layout{
   CompareLayout::ReferenceSolidCandidateWire=>(i==0,i==1),
   CompareLayout::ReferenceWireCandidateSolid=>(i==1,i==0),
   CompareLayout::Shared=>(self.solid,self.wireframe),
  }
 }
 pub fn style_label(self,i:usize)->&'static str{
  match self.style(i){(true,true)=>"Solid + wire",(true,false)=>"Solid",(false,true)=>"Wireframe",_=>"Points"}
 }
}
pub const SIZE:u32=640;
const BEFORE:[u8;3]=[96,165,250];
const AFTER:[u8;3]=[251,191,36];
const BG:[u8;3]=[49,51,55];
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
/// D50 Lab -> D50 XYZ -> Bradford-adapted linear sRGB -> sRGB.
/// Display gamut clipping is intentional; the numerical Lab is unchanged.
/// This is an uncalibrated visualization, not a monitor soft proof.
pub fn lab_rgb(v:[f32;3])->[u8;3]{
 let fy=(v[0]+16.)/116.;let fx=fy+v[1]/500.;let fz=fy-v[2]/200.;
 let inv=|t:f32|if t>6./29.{t*t*t}else{3.*(6_f32/29.).powi(2)*(t-4./29.)};
 let [x,y,z]=[0.9642*inv(fx),inv(fy),0.8249*inv(fz)];
 let rgb=[3.1338561*x-1.6168667*y-0.4906146*z,-0.9787684*x+1.9161415*y+0.0334540*z,0.0719453*x-0.2289914*y+1.4052427*z];
 rgb.map(|v|{let v=v.clamp(0.,1.);let s=if v<=0.0031308{12.92*v}else{1.055*v.powf(1./2.4)-0.055};(255.*s).round() as u8})
}
fn triangles(c:&Cloud)->&[[[f32;3];3]]{c.surface.as_ref().map(|s|s.triangles.as_slice()).unwrap_or(&[])}
fn slice_segment(t:[[f32;3];3],light:f32)->Option<[[f32;3];2]>{
 let mut points=[[0.;3];3];let mut n=0;
 for (a,b) in [(t[0],t[1]),(t[1],t[2]),(t[2],t[0])]{
  if (a[0]<=light&&b[0]>light)||(b[0]<=light&&a[0]>light){
   let f=(light-a[0])/(b[0]-a[0]);points[n]=[light,a[1]+(b[1]-a[1])*f,a[2]+(b[2]-a[2])*f];n+=1;
  }
 }
 if n==2{Some([points[0],points[1]])}else{None}
}
struct Layer{depth:Vec<f32>,color:Vec<[u8;3]>}
impl Layer{
 fn new()->Self{Self{depth:vec![f32::NEG_INFINITY;(SIZE*SIZE) as usize],color:vec![[0;3];(SIZE*SIZE) as usize]}}
 fn pixel(&mut self,x:i32,y:i32,z:f32,color:[u8;3]){
  if x<0||y<0||x>=SIZE as i32||y>=SIZE as i32{return}
  let i=y as usize*SIZE as usize+x as usize;
  if z>=self.depth[i]{self.depth[i]=z;self.color[i]=color;}
 }
 fn triangle(&mut self,p:[[f32;3];3],colors:[[u8;3];3]){
  let edge=|a:[f32;3],b:[f32;3],x:f32,y:f32|(b[0]-a[0])*(y-a[1])-(b[1]-a[1])*(x-a[0]);
  let area=edge(p[0],p[1],p[2][0],p[2][1]);
  if area.abs()<0.00001{return}
  let bound=|axis:usize,high:bool|{let v=if high{p.iter().map(|p|p[axis]).fold(f32::NEG_INFINITY,f32::max).ceil()}else{p.iter().map(|p|p[axis]).fold(f32::INFINITY,f32::min).floor()};(v as i32).clamp(0,SIZE as i32-1)};
  for y in bound(1,false)..=bound(1,true){for x in bound(0,false)..=bound(0,true){
   let (fx,fy)=(x as f32+0.5,y as f32+0.5);
   let a=edge(p[1],p[2],fx,fy)/area;let b=edge(p[2],p[0],fx,fy)/area;let c=1.-a-b;
   if a>=-0.00001&&b>=-0.00001&&c>=-0.00001{
    let weights=[a,b,c];let z=(0..3).map(|j|weights[j]*p[j][2]).sum();
    let color=std::array::from_fn(|i|(0..3).map(|j|weights[j]*colors[j][i] as f32).sum::<f32>().round().clamp(0.,255.) as u8);
    self.pixel(x,y,z,color);
   }
  }}
 }
 fn wire(&mut self,a:[f32;3],b:[f32;3],ca:[u8;3],cb:[u8;3],solid:bool){
  let n=((a[0]-b[0]).abs().max((a[1]-b[1]).abs()).ceil() as usize).clamp(1,2048);
  for i in 0..=n{
   let t=i as f32/n as f32;let x=(a[0]+(b[0]-a[0])*t).round() as i32;let y=(a[1]+(b[1]-a[1])*t).round() as i32;
   let z=a[2]+(b[2]-a[2])*t;
   let mut c=std::array::from_fn(|j|(ca[j] as f32+(cb[j] as f32-ca[j] as f32)*t).round() as u8);
   if solid{c=c.map(|v|(v as f32*0.68).round() as u8);}
   if x>=0&&y>=0&&x<SIZE as i32&&y<SIZE as i32{
    let idx=y as usize*SIZE as usize+x as usize;
    if !solid||z+1.0>=self.depth[idx]{self.pixel(x,y,z+if solid{1.0}else{0.},c);}
   }
  }
 }
}

// Two families of true surface intersections form a quad-style wire grid:
// constant L* rings and vertical hue meridians. No internal triangle edges.
// At cusps and disconnected regions this is not an all-quad topology.
fn plane_segment(t:[[f32;3];3],value:impl Fn([f32;3])->f32)->Option<[[f32;3];2]>{
 let mut p=[[0.;3];3];let mut n=0;
 for (a,b) in [(t[0],t[1]),(t[1],t[2]),(t[2],t[0])]{
  let (u,v)=(value(a),value(b));
  if (u<=0.&&v>0.)||(v<=0.&&u>0.){
   let f=u/(u-v);p[n]=std::array::from_fn(|j|a[j]+(b[j]-a[j])*f);n+=1;
  }
 }
 if n==2{Some([p[0],p[1]])}else{None}
}
fn quad_grid(mesh:&[[[f32;3];3]])->Vec<[[f32;3];2]>{
 let mut out=Vec::new();
 for light in (5..100).step_by(5){out.extend(mesh.iter().filter_map(|&t|slice_segment(t,light as f32)));}
 for k in 0..24{
  let angle=k as f32*std::f32::consts::TAU/24.;let (s,c)=angle.sin_cos();
  for &t in mesh{
   if let Some([mut a,mut b])=plane_segment(t,|v|v[1]*s-v[2]*c){
    let (ra,rb)=(a[1]*c+a[2]*s,b[1]*c+b[2]*s);
    if ra<0.&&rb<0.{continue}
    if (ra<0.)!=(rb<0.){
     let f=ra/(ra-rb);let p=std::array::from_fn(|j|a[j]+(b[j]-a[j])*f);
     if ra<0.{a=p}else{b=p}
    }
    out.push([a,b]);
   }
  }
 }
 out
}
fn boundary(l:&Layer,i:usize)->bool{
 if !l.depth[i].is_finite(){return false}
 let n=SIZE as usize;let x=i%n;let y=i/n;
 x==0||y==0||x+1==n||y+1==n||[i-1,i+1,i-n,i+n].into_iter().any(|j|!l.depth[j].is_finite())
}
fn paint_outline(img:&mut RgbImage,l:&Layer,alpha:u16){
 for i in 0..l.depth.len(){if boundary(l,i){paint(img,(i%SIZE as usize) as i32,(i/SIZE as usize) as i32,l.color[i],alpha);}}
}

// Minimal raster text, no platform font dependency. Labels stay upright.
fn label(img:&mut RgbImage,p:[f32;2],s:&str){
 let glyph=|c:char|match c{
 '0'=>[7,5,5,5,7],'1'=>[2,6,2,2,7],'2'=>[7,1,7,4,7],'3'=>[7,1,7,1,7],
 '4'=>[5,5,7,1,1],'5'=>[7,4,7,1,7],'6'=>[7,4,7,5,7],'7'=>[7,1,1,1,1],
 '8'=>[7,5,7,5,7],'9'=>[7,5,7,1,7],'-'=>[0,0,7,0,0],
 'L'=>[4,4,4,4,7],'a'=>[0,7,1,7,7],'b'=>[4,4,6,5,6],'*'=>[0,5,2,5,0],_=>[0;5]};
 let x=(p[0].round() as i32).clamp(2,SIZE as i32-8*s.chars().count() as i32-2);let y=(p[1].round() as i32).clamp(2,SIZE as i32-12);
 for (k,c) in s.chars().enumerate(){for (j,row) in glyph(c).into_iter().enumerate(){for i in 0..3{
  if row&(1<<(2-i))!=0{for dy in 0..2{for dx in 0..2{paint(img,x+k as i32*8+i*2+dx,y+j as i32*2+dy,[225,227,230],255);}}}
 }}}
}
fn grid3d(img:&mut RgbImage,p:&impl Fn([f32;3])->[f32;2],o:ViewOptions,a:f32,b:f32){
 // Back walls are chosen by camera direction. Floors and walls are
 // rasterized before data so they never hide the profile.
 let a_back=if project([50.,a,0.],o.yaw,o.pitch)[2]<0.{a}else{-a};
 let b_back=if project([50.,0.,b],o.yaw,o.pitch)[2]<0.{b}else{-b};
 let mut wall=Layer::new();
 for quad in [
 [[0.,-a,-b],[0.,a,-b],[0.,a,b],[0.,-a,b]],
 [[0.,a_back,-b],[100.,a_back,-b],[100.,a_back,b],[0.,a_back,b]],
 [[0.,-a,b_back],[100.,-a,b_back],[100.,a,b_back],[0.,a,b_back]]
 ]{
  let q=quad.map(|v|{let xy=p(v);[xy[0],xy[1],project(v,o.yaw,o.pitch)[2]]});
  wall.triangle([q[0],q[1],q[2]],[[69,71,75];3]);wall.triangle([q[0],q[2],q[3]],[[69,71,75];3]);
 }
 for y in 0..SIZE{for x in 0..SIZE{let i=(y*SIZE+x) as usize;if wall.depth[i].is_finite(){paint(img,x as i32,y as i32,wall.color[i],255);}}}
 let c=[105,108,113];let edge=[167,170,175];
 let step_a=if a>80.{20.}else{10.};let step_b=if b>80.{20.}else{10.};
 for k in -(a/step_a).floor() as i32..=(a/step_a).floor() as i32{
  let v=k as f32*step_a;
  line(img,p([0.,v,-b]),p([0.,v,b]),c,0,255);
  line(img,p([0.,v,b_back]),p([100.,v,b_back]),c,0,255);
  label(img,p([0.,v,-b_back]),&format!("{v:.0}"));
 }
 for k in -(b/step_b).floor() as i32..=(b/step_b).floor() as i32{
  let v=k as f32*step_b;
  line(img,p([0.,-a,v]),p([0.,a,v]),c,0,255);
  line(img,p([0.,a_back,v]),p([100.,a_back,v]),c,0,255);
  label(img,p([0.,-a_back,v]),&format!("{v:.0}"));
 }
 for l in (0..=100).step_by(10){
  let v=l as f32;
  line(img,p([v,a_back,-b]),p([v,a_back,b]),c,0,255);
  line(img,p([v,-a,b_back]),p([v,a,b_back]),c,0,255);
  let mut xy=p([v,a_back,-b_back]);xy[0]-=27.;label(img,xy,&l.to_string());
 }
 for (v,w) in [
 ([0.,-a,-b],[0.,a,-b]),([0.,a,-b],[0.,a,b]),([0.,a,b],[0.,-a,b]),([0.,-a,b],[0.,-a,-b]),
 ([0.,a_back,-b],[100.,a_back,-b]),([0.,a_back,b],[100.,a_back,b]),
 ([0.,-a,b_back],[100.,-a,b_back]),([0.,a,b_back],[100.,a,b_back]),
 ([100.,a_back,-b],[100.,a_back,b]),([100.,-a,b_back],[100.,a,b_back])
 ]{line(img,p(v),p(w),edge,0,255);}
 let mut l=p([100.,a_back,-b_back]);l[1]-=15.;label(img,l,"L*");
 label(img,p([0.,a,-b_back]),"a*");label(img,p([0.,-a_back,b]),"b*");
}

pub fn render(data:Option<&Overlay>,o:ViewOptions)->Rendered{
 let mut img=RgbImage::from_pixel(SIZE,SIZE,Rgb(BG));
 let clouds:[Option<&Cloud>;2]=[data.and_then(|d|d.reference.as_ref().ok()),data.and_then(|d|d.candidate.as_ref()).and_then(|d|d.as_ref().ok())];
 // Fit all points (not only visible ones) in a rotation-invariant frame.
 // This avoids rescaling when the user toggles Before/After or rotates.
 let extent=clouds.iter().flatten().flat_map(|c|c.points.iter()).filter(|v|v.iter().all(|x|x.is_finite())).map(|v|if o.three_d{((v[0]-50.).powi(2)+v[1].powi(2)+v[2].powi(2)).sqrt()}else{v[1].abs().max(v[2].abs())}).fold(20f32,f32::max);
 let mut a_extent=20f32;let mut b_extent=20f32;
 for c in clouds.iter().flatten(){for v in &c.points{a_extent=a_extent.max(v[1].abs());b_extent=b_extent.max(v[2].abs());}}
 let a_extent=((a_extent+8.)/10.).ceil()*10.;let b_extent=((b_extent+8.)/10.).ceil()*10.;
 let box_extent=(50f32.powi(2)+a_extent.powi(2)+b_extent.powi(2)).sqrt();
 let range=(if o.three_d{extent.max(box_extent)*1.12}else{extent*1.18}/10.).ceil()*10.;
 let zoom=if o.zoom.is_finite(){o.zoom.clamp(0.5,3.)}else{1.};
 let scale=(SIZE as f32/2.-28.)/range*zoom;
 let at=|v:[f32;3]|[SIZE as f32/2.+v[0]*scale,SIZE as f32/2.-v[1]*scale];
 let p=|v:[f32;3]|at(projected(v,o));
 let step=if range>100.{20.}else if range>50.{10.}else{5.};
 let tick_count=(range/step).floor() as i32;
 // The axis/grid is always rendered, including before a profile is opened.
 if o.three_d{grid3d(&mut img,&p,o,a_extent,b_extent);}
 if !o.three_d{
  for i in -tick_count..=tick_count{
   let v=i as f32*step;
   line(&mut img,p([50.,v,-range]),p([50.,v,range]),[88,91,96],0,255);
   line(&mut img,p([50.,-range,v]),p([50.,range,v]),[88,91,96],0,255);
  }
 }
 if !o.three_d{
 let axes=[([50.,-range,0.],[50.,range,0.],[232,121,121]),([50.,0.,-range],[50.,0.,range],[102,187,138]),([0.,0.,0.],[100.,0.,0.],[229,231,235])];
 for (i,(a,b,c)) in axes.into_iter().enumerate(){
  if i==2&&!o.three_d{continue}
  line(&mut img,p(a),p(b),c,1,215);
  for t in 0..=10{let f=t as f32/10.;let v=std::array::from_fn(|j|a[j]+(b[j]-a[j])*f);dot(&mut img,p(v),c,3,220);}
  dot(&mut img,p(b),c,5,255);
 }
 }
 let mut visible=[0;2];
 let mut layers:[Option<Layer>;2]=[None,None];
 let mut styles=[(false,false);2];
 let mut outline_only=[false;2];
 for (i,cloud) in clouds.into_iter().enumerate(){
  if !(if i==0{o.reference}else{o.candidate}){continue}
  let Some(c)=cloud else{continue};let (solid,wire)=o.style(i);styles[i]=(solid,wire);let tint=if i==0{BEFORE}else{AFTER};
  let color=|lab:[f32;3]|if o.profile_colors{lab_rgb(lab)}else{tint};
  let screen=|v:[f32;3]|{let q=projected(v,o);let xy=at(q);[xy[0],xy[1],if o.three_d{q[2]}else{v[0]}]};
  let points=||c.points.iter().copied().filter(|v|v.iter().all(|v|v.is_finite())&&in_slice(*v,o));
  visible[i]=points().count();let mut layer=Layer::new();
  let mesh=if solid||wire||o.floor_contour&&o.three_d{triangles(c)}else{&[]};
  // One union projection per visible profile. A disconnected footprint may
  // still have several components, but there are no stacked L* contours.
  if o.three_d&&o.floor_contour&&!mesh.is_empty(){
   let mut floor=Layer::new();
   for &t in mesh{
    let q=t.map(|v|{let xy=p([0.,v[1],v[2]]);[xy[0],xy[1],v[0]]});
    floor.triangle(q,t.map(color));
   }
   paint_outline(&mut img,&floor,205);
  }
  let mesh=if solid||wire{mesh}else{&[]};
  if !mesh.is_empty(){
   if !o.three_d&&o.slice{
    if solid||wire{
     let segs:Vec<_>=mesh.iter().filter_map(|&t|slice_segment(t,o.light)).map(|s|s.map(screen)).collect();
     let mut xs=Vec::new();
     for y in 0..SIZE as i32{
      let fy=y as f32+0.5;xs.clear();
      for [a,b] in &segs{if (a[1]<=fy&&b[1]>fy)||(b[1]<=fy&&a[1]>fy){xs.push(a[0]+(b[0]-a[0])*(fy-a[1])/(b[1]-a[1]));}}
      xs.sort_by(f32::total_cmp);
      for pair in xs.chunks_exact(2){for x in (pair[0].ceil() as i32).max(0)..=(pair[1].floor() as i32).min(SIZE as i32-1){
       let lab=[o.light,(x as f32+0.5-SIZE as f32/2.)/scale,(SIZE as f32/2.-fy)/scale];
       layer.pixel(x,y,o.light,color(lab));
      }}
     }
    }
    outline_only[i]=wire&&!solid;
   }else if !o.three_d{
    // Project union of surface triangles, retaining nonconvex silhouettes
    // and holes rather than drawing all triangulation edges.
    for &t in mesh{layer.triangle(t.map(screen),t.map(color));}
    outline_only[i]=wire&&!solid;
   }else{
    if solid{for &t in mesh{layer.triangle(t.map(screen),t.map(color));}}
    if wire{for [a,b] in quad_grid(mesh){layer.wire(screen(a),screen(b),color(a),color(b),solid);}}
   }
  }else{
   // Explicitly reported by UI as points fallback if no surface is found.
   for v in points(){let p=screen(v);let c=color(v);for dx in -3..=3{for dy in -3..=3{if dx*dx+dy*dy<=9{layer.pixel(p[0].round() as i32+dx,p[1].round() as i32+dy,p[2],c);}}}}
  }
  layers[i]=Some(layer);
 }
 // Per-profile Z-buffer prevents the far surface from painting over near
 // triangles. When both profiles overlap, blend in depth order.
 for y in 0..SIZE{for x in 0..SIZE{
  let i=(y*SIZE+x) as usize;
  let a=layers[0].as_ref().filter(|l|l.depth[i].is_finite()&&(!outline_only[0]||boundary(l,i)));
  let b=layers[1].as_ref().filter(|l|l.depth[i].is_finite()&&(!outline_only[1]||boundary(l,i)));
  let alpha=|index:usize,l:&Layer|if !o.three_d&&styles[index].0&&!(styles[index].1&&boundary(l,i)){u16::from(o.opacity_2d.min(100))*255/100}else{255};
  match (a,b){
   (Some(a),Some(b))=>{
    // For mixed comparison, draw the solid unchanged and overlay wire
    // regardless of depth, preserving the enclosing comparison cage.
    if styles[0].0&&!styles[1].0&&styles[1].1{paint(&mut img,x as i32,y as i32,a.color[i],alpha(0,a));paint(&mut img,x as i32,y as i32,b.color[i],220);}
    else if styles[1].0&&!styles[0].0&&styles[0].1{paint(&mut img,x as i32,y as i32,b.color[i],alpha(1,b));paint(&mut img,x as i32,y as i32,a.color[i],220);}
    else{let (far,near,fi,ni)=if a.depth[i]<=b.depth[i]{(a,b,0,1)}else{(b,a,1,0)};paint(&mut img,x as i32,y as i32,far.color[i],alpha(fi,far));paint(&mut img,x as i32,y as i32,near.color[i],alpha(ni,near)*155/255);}
   },
   (Some(l),None)=>paint(&mut img,x as i32,y as i32,l.color[i],alpha(0,l)),
   (None,Some(l))=>paint(&mut img,x as i32,y as i32,l.color[i],alpha(1,l)),
   _=>{}
  }
 }}
 Rendered{image:img,visible,units_per_half:range/zoom}
}
#[cfg(test)]mod tests{
 use super::*;
 fn example()->Overlay{
  let points=vec![[30.,-4.,-40.],[50.,12.,20.],[98.,0.,0.],[70.,22.,-25.]];
  Overlay{reference:Ok(Cloud{points:points.clone(),sampled:4,errors:vec![0.;4],surface:None}),candidate:Some(Ok(Cloud{points,sampled:4,errors:vec![0.;4],surface:None}))}
 }
 #[test]fn mixed_layout_preserves_solid_and_wire_overlay(){
  let mut d=example();
  let t=crate::gamut_surface::Surface{triangles:vec![[[50.,-20.,-20.],[50.,20.,-20.],[50.,0.,20.]]],resolution:25,volume:0.};
  d.reference.as_mut().unwrap().surface=Some(t.clone());
  d.candidate.as_mut().unwrap().as_mut().unwrap().surface=Some(t);
  let o=ViewOptions{three_d:false,profile_colors:false,..Default::default()};
  assert_eq!(o.style(0),(true,false));assert_eq!(o.style(1),(false,true));
  let img=render(Some(&d),o).image;
  assert_eq!(img.get_pixel(SIZE/2,SIZE/2).0,BEFORE);
  assert!(img.pixels().any(|p|p[0]>200&&p[1]>100&&p[2]<100));
  let reverse=ViewOptions{layout:CompareLayout::ReferenceWireCandidateSolid,..o};
  assert_eq!(render(Some(&d),reverse).image.get_pixel(SIZE/2,SIZE/2).0,AFTER);
 }
 #[test]fn floor_contours_toggle(){
  let mut d=example();d.reference.as_mut().unwrap().surface=Some(crate::gamut_surface::Surface{triangles:vec![[[10.,-10.,-10.],[90.,20.,-20.],[90.,0.,20.]]],resolution:25,volume:0.});
  let o=ViewOptions{candidate:false,..Default::default()};
  assert_ne!(render(Some(&d),o).image.as_raw(),render(Some(&d),ViewOptions{floor_contour:false,..o}).image.as_raw());
 }
 #[test]fn lab_color_has_expected_neutrals_and_hue(){
  let w=lab_rgb([100.,0.,0.]);assert!(w.iter().all(|&v|v>=254));assert_eq!(lab_rgb([0.,0.,0.]),[0;3]);
  let red=lab_rgb([55.,65.,40.]);assert!(red[0]>red[1]&&red[0]>red[2]);
 }
 #[test]fn solid_triangle_and_depth(){
  let mut l=Layer::new();let p=[[10.,10.,2.],[100.,10.,2.],[10.,100.,2.]];
  l.triangle(p,[[255,0,0];3]);l.triangle(p.map(|mut p|{p[2]=1.;p}),[[0,0,255];3]);
  assert_eq!(l.color[20*SIZE as usize+20],[255,0,0]);assert!(l.depth[20*SIZE as usize+20].is_finite());
 }
 #[test]fn solid_surface_fills_interior(){
  let mut d=example();
  let c=d.reference.as_mut().unwrap();
  c.surface=Some(crate::gamut_surface::Surface{triangles:vec![[[50.,-20.,-20.],[50.,20.,-20.],[50.,0.,20.]]],resolution:25,volume:0.});
  let o=ViewOptions{three_d:false,candidate:false,profile_colors:false,solid:true,..Default::default()};
  let result=render(Some(&d),o);
  assert_eq!(result.image.get_pixel(SIZE/2,SIZE/2).0,BEFORE);
 }
 #[test]fn intersections_and_borrowed_mesh(){
  let t=[[20.,0.,0.],[80.,10.,0.],[80.,0.,10.]];
  assert_eq!(slice_segment(t,50.),plane_segment(t,|v|v[0]-50.));
  assert!(slice_segment(t,10.).is_none());assert!(plane_segment(t,|v|v[0]-90.).is_none());
  let mut d=example();let c=d.reference.as_mut().unwrap();
  c.surface=Some(crate::gamut_surface::Surface{triangles:vec![t],resolution:25,volume:0.});
  assert_eq!(triangles(c).as_ptr(),c.surface.as_ref().unwrap().triangles.as_ptr());
 }
 #[test]fn slice_crosses_triangle(){let r=slice_segment([[20.,0.,0.],[80.,10.,0.],[80.,0.,10.]],50.).unwrap();assert_eq!(r,[[50.,5.,0.],[50.,0.,5.]]);}
 #[test]fn both_wire_preserves_rgb_ratios(){
  let mut layer=Layer::new();
  layer.wire([10.,10.,0.],[20.,10.,0.],[200,100,50],[200,100,50],true);
  assert_eq!(layer.color[10*SIZE as usize+15],[136,68,34]);
 }
 #[test]fn quad_grid_uses_surface_sections(){
  let t=[[[10.,10.,0.],[90.,10.,10.],[90.,-10.,0.]]];
  let grid=quad_grid(&t);assert!(!grid.is_empty());
  assert!(grid.iter().any(|s|(s[0][0]-s[1][0]).abs()<0.001));
 }
 #[test]fn outline_and_opacity_2d(){
  let mut d=example();d.reference.as_mut().unwrap().surface=Some(crate::gamut_surface::Surface{triangles:vec![[[50.,-20.,-20.],[50.,20.,-20.],[50.,0.,20.]]],resolution:25,volume:0.});
  let o=ViewOptions{three_d:false,candidate:false,profile_colors:false,..Default::default()};
  let solid=render(Some(&d),o);let clear=render(Some(&d),ViewOptions{opacity_2d:0,..o});
  let outline=render(Some(&d),ViewOptions{layout:CompareLayout::Shared,solid:false,wireframe:true,..o});
  assert_eq!(solid.image.get_pixel(SIZE/2,SIZE/2).0,BEFORE);
  assert_ne!(clear.image.get_pixel(SIZE/2,SIZE/2).0,BEFORE);
  assert_eq!(clear.image.get_pixel(SIZE/2,SIZE/2),outline.image.get_pixel(SIZE/2,SIZE/2));
 }
 #[test]fn grid_always_visible(){for three_d in [false,true]{let r=render(None,ViewOptions{three_d,..Default::default()});assert!(r.image.pixels().any(|p|p.0!=BG));}}
 #[test]fn actual_pixels_change_in_both_views(){
  let d=example();
  for three_d in [false,true]{
   let o=ViewOptions{three_d,candidate:false,profile_colors:false,solid:false,layout:CompareLayout::Shared,..Default::default()};
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
  for three_d in [false,true]{let r=render(Some(&data),ViewOptions{three_d,profile_colors:false,solid:false,layout:CompareLayout::Shared,..Default::default()});assert!(r.visible[0]>5);assert!(r.image.pixels().any(|p|p[2]>160&&p[2]>p[0]));}
 }
}
