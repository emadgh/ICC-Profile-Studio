//! Nonconvex iso-surface of sampled round-trip color reproduction error.
//! Marching tetrahedra on a regular Lab lattice, with shared face diagonals.
//! No convex closure; this is a tolerance-dependent estimate, not a
//! characterization of every physically attainable device color.
pub const TOLERANCE:f64=2.;
#[derive(Clone,Debug)]
pub struct Surface{pub triangles:Vec<[[f32;3];3]>,pub resolution:usize}
pub fn extract(field:&[f64],n:usize,lo:[f32;3],hi:[f32;3],threshold:f64)->Option<Surface>{
 if n<3||field.len()!=n.pow(3)||field.iter().any(|v|!v.is_finite()){return None}
 let idx=|x:usize,y:usize,z:usize|(x*n+y)*n+z;
 let position=|x:usize,y:usize,z:usize|std::array::from_fn(|i|lo[i]+(hi[i]-lo[i])*[x,y,z][i] as f32/(n-1) as f32);
 let corners=[[0,0,0],[1,0,0],[1,1,0],[0,1,0],[0,0,1],[1,0,1],[1,1,1],[0,1,1]];
 let tetra=[[0,5,1,6],[0,1,2,6],[0,2,3,6],[0,3,7,6],[0,7,4,6],[0,4,5,6]];
 let mut triangles=Vec::new();
 for x in 0..n-1{for y in 0..n-1{for z in 0..n-1{
  let values=corners.map(|[a,b,c]|field[idx(x+a,y+b,z+c)]);
  if values.iter().all(|&v|v<=threshold)||values.iter().all(|&v|v>threshold){continue}
  let pos=corners.map(|[a,b,c]|position(x+a,y+b,z+c));
  let cross=|a:usize,b:usize|{let t=((threshold-values[a])/(values[b]-values[a])).clamp(0.,1.) as f32;std::array::from_fn(|i|pos[a][i]+(pos[b][i]-pos[a][i])*t)};
  for t in tetra{
   let mut inside=[0usize;4];let mut outside=[0usize;4];let (mut ni,mut no)=(0,0);
   for i in t{if values[i]<=threshold{inside[ni]=i;ni+=1;}else{outside[no]=i;no+=1;}}
   match ni{
    1=>triangles.push([cross(inside[0],outside[0]),cross(inside[0],outside[1]),cross(inside[0],outside[2])]),
    3=>triangles.push([cross(outside[0],inside[0]),cross(outside[0],inside[1]),cross(outside[0],inside[2])]),
    2=>{let a=cross(inside[0],outside[0]);let b=cross(inside[0],outside[1]);let c=cross(inside[1],outside[0]);let d=cross(inside[1],outside[1]);triangles.push([a,b,c]);triangles.push([b,d,c]);},
    _=>{}
   }
  }
 }}}
 if triangles.is_empty(){None}else{Some(Surface{triangles,resolution:n})}
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn preserves_separated_regions(){
  let n=25usize;let mut f=Vec::new();
  for x in 0..n{for y in 0..n{for z in 0..n{
   let d=|c:f64|((x as f64-c).powi(2)+(y as f64-12.).powi(2)+(z as f64-12.).powi(2)).sqrt();
   f.push(d(5.).min(d(19.)));
  }}}
  let s=extract(&f,n,[0.;3],[24.;3],3.5).unwrap();
  assert!(s.triangles.len()>100);
  assert!(s.triangles.iter().all(|t|t.iter().all(|v|v[0]<9.)||t.iter().all(|v|v[0]>15.)));
 }
 #[test]fn empty_has_no_surface(){assert!(extract(&vec![3.;27],3,[0.;3],[2.;3],2.).is_none());}
}
