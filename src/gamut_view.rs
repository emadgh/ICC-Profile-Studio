//! Two independent GPUI presentation backends for the same tested raster.
//! Default: ordinary colored Div rectangles (no Canvas or img dependency).
//! Alternate: synchronous, predecoded BGRA RenderImage (no async asset decode).
use gpui::{prelude::*,*};
use std::sync::Arc;
use color_convert_rust::{gamut::Overlay,gamut_raster};
pub use gamut_raster::ViewOptions;
pub const DISPLAY:f32=320.;
#[derive(Default)]
pub struct PreviewCache{
 key:Option<(u64,bool,ViewOptions)>,
 spans:Vec<gamut_raster::Span>,
 image:Option<Arc<RenderImage>>,
 label:String,
}
impl PreviewCache{
 fn update(&mut self,data:Option<&Overlay>,revision:u64,o:ViewOptions){
  let key=(revision,data.is_some(),o);
  if self.key==Some(key){return}
  let r=gamut_raster::render(data,o);
  self.label=format!("Visible points: {} before / {} after · ±{:.0} Lab units",r.visible[0],r.visible[1],r.units_per_half);
  self.spans=gamut_raster::native_spans(&r.image);
  let mut buffer=image::RgbaImage::new(gamut_raster::SIZE,gamut_raster::SIZE);
  for (x,y,p) in r.image.enumerate_pixels(){buffer.put_pixel(x,y,image::Rgba([p[2],p[1],p[0],255]));}
  self.image=Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])));
  self.key=Some(key);
 }
}
pub fn plot(data:Option<&Overlay>,revision:u64,o:ViewOptions,cache:&mut PreviewCache,use_image:bool)->Div{
 cache.update(data,revision,o);
 let frame=div().relative().w(px(DISPLAY)).h(px(DISPLAY)).min_h(px(DISPLAY)).flex_shrink_0().overflow_hidden().bg(rgb(0x313337));
 let frame=if use_image{
  if let Some(image)=cache.image.clone(){frame.child(img(image).w(px(DISPLAY)).h(px(DISPLAY)).flex_shrink_0())}
  else{frame.child("Direct BGRA image unavailable")}
 }else{
  let cell=DISPLAY/gamut_raster::CELL_GRID as f32;
  frame.children(cache.spans.iter().map(|s|div().absolute().left(px(s.x as f32*cell)).top(px(s.y as f32*cell)).w(px(s.width as f32*cell)).h(px(cell)).bg(rgb(s.rgb))))
 };
 // Border and three simple swatches are deliberately independent of plot data.
 // If these are missing, the entire panel is clipped or not rendered.
 div().flex().flex_col().items_center().gap_1().w_full().flex_shrink_0()
  .child(div().p(px(1.)).bg(rgb(0x536780)).flex_shrink_0().child(frame))
  .child(div().flex().gap_1().items_center().child(div().w(px(12.)).h(px(8.)).bg(rgb(0xe87979))).child(div().w(px(12.)).h(px(8.)).bg(rgb(0x66bb8a))).child(div().w(px(12.)).h(px(8.)).bg(rgb(0xe5e7eb))).child(div().text_xs().child(if use_image{"BGRA renderer · R4"}else{"Native cells renderer · R4"})))
  .child(div().text_xs().child(cache.label.clone()))
  .child(div().text_xs().child(format!("{} native spans · {} view",cache.spans.len(),if o.three_d{"3D"}else{"2D"})))
}
