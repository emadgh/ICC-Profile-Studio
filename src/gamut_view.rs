//! GPUI BGRA-only presentation. Raster shared with PNG export.
use gpui::{prelude::*,*};
use std::sync::Arc;
use color_convert_rust::{gamut::Overlay,gamut_raster};
pub use gamut_raster::ViewOptions;
#[derive(Default)]
pub struct PreviewCache{
 key:Option<(u64,bool,ViewOptions)>,image:Option<Arc<RenderImage>>,label:String,
 wire_revision:Option<(u64,bool)>,wire:[Option<Vec<[[f32;3];2]>>;2],wire_attempted:[bool;2],
}
impl PreviewCache{
 fn update(&mut self,data:Option<&Overlay>,revision:u64,o:ViewOptions){
  let key=(revision,data.is_some(),o);
  if self.key==Some(key){return}
  let revision_key=(revision,data.is_some());
  if self.wire_revision!=Some(revision_key){self.wire_revision=Some(revision_key);self.wire=[None,None];self.wire_attempted=[false;2];}
  let clouds=[data.and_then(|d|d.reference.as_ref().ok()),data.and_then(|d|d.candidate.as_ref()).and_then(|d|d.as_ref().ok())];
  for i in 0..2{
   if o.three_d&&o.style(i).1&&(if i==0{o.reference}else{o.candidate})&&!self.wire_attempted[i]{
    self.wire_attempted[i]=true;
    self.wire[i]=clouds[i].and_then(gamut_raster::prepare_wire);
   }
  }
  let r=gamut_raster::render_prepared(data,o,[self.wire[0].as_deref(),self.wire[1].as_deref()]);
  self.label=format!("{} reference / {} candidate points · ±{:.0} Lab",r.visible[0],r.visible[1],r.units_per_half);
  let mut bgra=Vec::with_capacity((gamut_raster::SIZE*gamut_raster::SIZE*4) as usize);
  for p in r.image.as_raw().chunks_exact(3){bgra.extend_from_slice(&[p[2],p[1],p[0],255]);}
  let buffer=image::RgbaImage::from_raw(gamut_raster::SIZE,gamut_raster::SIZE,bgra).expect("fixed raster dimensions");
  self.image=Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])));
  self.key=Some(key);
 }
}
pub fn plot(data:Option<&Overlay>,revision:u64,o:ViewOptions,cache:&mut PreviewCache,display:f32)->Div{
 cache.update(data,revision,o);
 let display=display.clamp(180.,1000.);
 let mut frame=div().w(px(display)).h(px(display)).flex_shrink_0().overflow_hidden().bg(rgb(0x313337));
 if let Some(image)=cache.image.clone(){frame=frame.child(img(image).w(px(display)).h(px(display)).flex_shrink_0());}
 div().flex().flex_col().items_center().gap_1().w_full().flex_shrink_0()
  .child(div().p(px(1.)).bg(rgb(0x555555)).flex_shrink_0().child(frame))
  .child(div().text_xs().text_color(rgb(0xa1a1aa)).child(cache.label.clone()))
}
