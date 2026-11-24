//! GPUI BGRA-only presentation. Raster shared with PNG export.
use gpui::{prelude::*,*};
use std::sync::Arc;
use color_convert_rust::{gamut::Overlay,gamut_raster};
pub use gamut_raster::ViewOptions;
#[derive(Default)]
pub struct PreviewCache{
 key:Option<(u64,bool,ViewOptions)>,image:Option<Arc<RenderImage>>,label:String,
}
impl PreviewCache{
 fn update(&mut self,data:Option<&Overlay>,revision:u64,o:ViewOptions){
  let key=(revision,data.is_some(),o);
  if self.key==Some(key){return}
  let r=gamut_raster::render(data,o);
  self.label=format!("{} reference / {} candidate points · ±{:.0} Lab",r.visible[0],r.visible[1],r.units_per_half);
  let mut buffer=image::RgbaImage::new(gamut_raster::SIZE,gamut_raster::SIZE);
  for (x,y,p) in r.image.enumerate_pixels(){buffer.put_pixel(x,y,image::Rgba([p[2],p[1],p[0],255]));}
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
  .child(div().p(px(1.)).bg(rgb(0x536780)).flex_shrink_0().child(frame))
  .child(div().text_xs().text_color(rgb(0x94a3b8)).child(cache.label.clone()))
}
