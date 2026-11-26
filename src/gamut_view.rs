//! GPUI presentation only; heavy raster and BGRA packing run on a worker.
use gpui::{prelude::*,*};
use std::sync::Arc;
use color_convert_rust::{gamut::Overlay,gamut_raster::{self,FrameGate,FrameKey}};
pub use gamut_raster::ViewOptions;
#[derive(Default)]
struct WireCache{
 revision:Option<(u64,bool)>,wire:[Option<Vec<[[f32;3];2]>>;2],attempted:[bool;2],
}
pub struct FrameWork{key:FrameKey,data:Option<Arc<Overlay>>,wire:WireCache}
#[derive(Clone,Copy)]
struct PreviewStats{visible:[usize;2],units:f32}
pub struct FrameResult{key:FrameKey,wire:WireCache,result:Result<(image::RgbaImage,PreviewStats),String>}
impl FrameWork{
 pub fn run(self)->FrameResult{
  let Self{key,data,mut wire}=self;
  // A worker panic must release the in-flight slot rather than freeze preview.
  let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{
   let o=key.options;let data=data.as_deref();let revision=(key.revision,key.present);
   if wire.revision!=Some(revision){wire=WireCache{revision:Some(revision),..Default::default()};}
   let clouds=[data.and_then(|d|d.reference.as_ref().ok()),data.and_then(|d|d.candidate.as_ref()).and_then(|d|d.as_ref().ok())];
   for i in 0..2{
    if o.three_d&&o.style(i).1&&(if i==0{o.reference}else{o.candidate})&&!wire.attempted[i]{
     wire.attempted[i]=true;wire.wire[i]=clouds[i].and_then(gamut_raster::prepare_wire);
    }
   }
   let r=gamut_raster::render_prepared(data,o,[wire.wire[0].as_deref(),wire.wire[1].as_deref()]);
   let stats=PreviewStats{visible:r.visible,units:r.units_per_half};
   let mut bgra=Vec::with_capacity((gamut_raster::SIZE*gamut_raster::SIZE*4) as usize);
   for p in r.image.as_raw().chunks_exact(3){bgra.extend_from_slice(&[p[2],p[1],p[0],255]);}
   let buffer=image::RgbaImage::from_raw(gamut_raster::SIZE,gamut_raster::SIZE,bgra).expect("fixed raster dimensions");
   (buffer,stats)
  })).map_err(|_|"Preview rendering failed".to_owned());
  if result.is_err(){wire=WireCache::default();}
  FrameResult{key,wire,result}
 }
}
#[derive(Default)]
pub struct PreviewCache{
 gate:FrameGate,wire:WireCache,image:Option<Arc<RenderImage>>,image_key:Option<FrameKey>,stats:Option<PreviewStats>,failed:bool,
}
impl PreviewCache{
 pub fn request(&mut self,data:Option<Arc<Overlay>>,revision:u64,o:ViewOptions)->Option<FrameWork>{
  let key=FrameKey{revision,present:data.is_some(),options:o};
  if self.image_key.is_some_and(|previous|!previous.same_dataset(key)){
   self.image=None;self.image_key=None;self.stats=None;self.failed=false;
  }
  if self.gate.request(key){Some(FrameWork{key,data,wire:std::mem::take(&mut self.wire)})}else{None}
 }
 pub fn complete(&mut self,frame:FrameResult){
  self.wire=frame.wire;
  if !self.gate.complete(frame.key){return}
  self.image_key=Some(frame.key);
  match frame.result{
   Ok((buffer,stats))=>{self.image=Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])));self.stats=Some(stats);self.failed=false;}
   Err(_)=>{self.image=None;self.stats=None;self.failed=true;}
  }
 }
}
pub fn plot(cache:&PreviewCache,display:f32)->Div{
 let display=display.clamp(180.,1000.);
 let mut frame=div().w(px(display)).h(px(display)).flex_shrink_0().overflow_hidden().bg(rgb(0x313337));
 if let Some(image)=cache.image.clone(){frame=frame.child(img(image).w(px(display)).h(px(display)).flex_shrink_0());}
 let mut p=div().flex().flex_col().items_center().gap_1().w_full().flex_shrink_0()
  .child(div().p(px(1.)).bg(rgb(0x555555)).flex_shrink_0().child(frame))
  .child(div().text_xs().text_color(rgb(0xa1a1aa)).child(cache.stats.map(|s|color_convert_rust::i18n::preview_summary(s.visible,s.units)).unwrap_or_default()));
 if cache.gate.pending(){p=p.child(div().text_xs().text_color(rgb(0xfb923c)).child(color_convert_rust::i18n::tr("Rendering preview…")));}
 else if cache.failed{p=p.child(div().text_xs().text_color(rgb(0xfca5a5)).child(color_convert_rust::i18n::tr("Preview rendering failed")));}
 p
}
