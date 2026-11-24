//! GPUI canvas: fixed shared axes, depth-sorted orthographic point cloud.
use gpui::{prelude::*,*};
use color_convert_rust::gamut::{Overlay,project};
#[derive(Clone,Copy)]
pub struct ViewOptions{pub three_d:bool,pub reference:bool,pub candidate:bool,pub slice:bool,pub light:f32,pub yaw:f32,pub pitch:f32,pub zoom:f32}
impl Default for ViewOptions{fn default()->Self{Self{three_d:true,reference:true,candidate:true,slice:false,light:50.,yaw:0.65,pitch:0.45,zoom:1.}}}
pub fn plot(data:&Overlay,o:ViewOptions)->impl IntoElement{
 let mut dots:Vec<([f32;3],u32)>=Vec::new();
 let mut add=|cloud:&color_convert_rust::gamut::Cloud,color:u32|{
  for &v in &cloud.points{
   if !o.three_d&&o.slice&&(v[0]-o.light).abs()>5.{continue}
   let coord=if o.three_d{project(v,o.yaw,o.pitch)}else{[v[1],v[2],0.]};
   dots.push((coord,color));
  }
 };
 if o.reference{if let Ok(c)=&data.reference{add(c,0x60a5fa);}}
 if o.candidate{if let Some(Ok(c))=&data.candidate{add(c,0xfbbf24);}}
 if o.three_d{dots.sort_by(|a,b|a.0[2].total_cmp(&b.0[2]));}
 let axes=[([50.,-128.,0.],[50.,127.,0.],0xe87979),([50.,0.,-128.],[50.,0.,127.],0x66bb8a),([0.,0.,0.],[100.,0.,0.],0xe5e7eb)];
 canvas(move|_,_,_|{},move|bounds,_,window,_|{
  window.paint_quad(fill(bounds,rgb(0x0b1220)));
  let center=point(bounds.origin.x+bounds.size.width/2.,bounds.origin.y+bounds.size.height/2.);
  let side=if bounds.size.width<bounds.size.height{bounds.size.width}else{bounds.size.height};
  let scale=side/360.*o.zoom;
  let at=|v:[f32;3]|point(center.x+scale*v[0],center.y-scale*v[1]);
  let plot_coord=|v:[f32;3]|if o.three_d{project(v,o.yaw,o.pitch)}else{[v[1],v[2],0.]};
  for (index,(start,end,color)) in axes.into_iter().enumerate(){
   if !o.three_d&&index==2{continue}
   let a=plot_coord(start);let b=plot_coord(end);
   for i in 0..=220{let t=i as f32/220.;let v=[a[0]+(b[0]-a[0])*t,a[1]+(b[1]-a[1])*t,0.];let p=at(v);
    if p.x>=bounds.origin.x&&p.x+px(1.)<=bounds.origin.x+bounds.size.width&&p.y>=bounds.origin.y&&p.y+px(1.)<=bounds.origin.y+bounds.size.height{
     window.paint_quad(fill(Bounds{origin:p,size:size(px(1.2),px(1.2))},rgb(color)));
    }
   }
  }
  for (v,c) in dots{
   let p=at(v);if p.x<bounds.origin.x||p.x+px(2.)>bounds.origin.x+bounds.size.width||p.y<bounds.origin.y||p.y+px(2.)>bounds.origin.y+bounds.size.height{continue}
   window.paint_quad(fill(Bounds{origin:p,size:size(px(2.),px(2.))},rgba((c<<8)|0x95)));
  }
 }).w_full().h(px(310.))
}
