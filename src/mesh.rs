extern crate cgmath;
extern crate stl_io;
extern crate tobj;

use std::error::Error;
use std::fmt;
use std::fs::File;
use std::path::Path;
use std::convert::TryInto;

#[derive(Copy, Clone)]
pub struct Vertex {
    position: stl_io::Vertex,
    //texcoords: [f32; 2],
}

implement_vertex!(Vertex, position);
//implement_vertex!(Vertex, position, texcoords);

#[derive(Copy, Clone)]
pub struct Normal {
    normal: stl_io::Normal,
}

implement_vertex!(Normal, normal);

pub struct BoundingBox {
    pub min: cgmath::Point3<f32>,
    pub max: cgmath::Point3<f32>,
}

impl BoundingBox {
    fn new(vert: &stl_io::Vertex) -> BoundingBox {
        BoundingBox {
            min: cgmath::Point3 {
                x: vert[0],
                y: vert[1],
                z: vert[2],
            },
            max: cgmath::Point3 {
                x: vert[0],
                y: vert[1],
                z: vert[2],
            },
        }
    }
    fn expand(&mut self, vert: &stl_io::Vertex) {
        if vert[0] < self.min.x {
            self.min.x = vert[0];
        } else if vert[0] > self.max.x {
            self.max.x = vert[0];
        }
        if vert[1] < self.min.y {
            self.min.y = vert[1];
        } else if vert[1] > self.max.y {
            self.max.y = vert[1];
        }
        if vert[2] < self.min.z {
            self.min.z = vert[2];
        } else if vert[2] > self.max.z {
            self.max.z = vert[2];
        }
    }
    pub fn center(&self) -> cgmath::Point3<f32> {
        cgmath::Point3 {
            x: (self.min.x + self.max.x) / 2.0,
            y: (self.min.y + self.max.y) / 2.0,
            z: (self.min.z + self.max.z) / 2.0,
        }
    }
    fn length(&self) -> f32 {
        self.max.x - self.min.x
    }
    fn width(&self) -> f32 {
        self.max.y - self.min.y
    }
    fn height(&self) -> f32 {
        self.max.z - self.min.z
    }
}

impl fmt::Display for BoundingBox {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "X: {}, {}", self.min.x, self.max.x)?;
        writeln!(f, "Y: {}, {}", self.min.y, self.max.y)?;
        writeln!(f, "Z: {}, {}", self.min.z, self.max.z)?;
        Ok(())
    }
}

pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub normals: Vec<Normal>,
    pub indices: Vec<usize>,
    pub bounds: BoundingBox,
    stl_had_normals: bool,
    obj: bool,
}

impl Mesh {
    pub fn from_obj(obj_file: &String) -> Result<Mesh, Box<Error>> {
        //let stl = stl_io::read_stl(&mut stl_file)?;
        //debug!("{:?}", stl);

        // Get starting point for finding bounding box

        let mut mesh = Mesh {
            vertices: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
            bounds: BoundingBox::new(&[f32::INFINITY; 3]),
            stl_had_normals: true,
            obj: true,
        };
        match tobj::load_obj(Path::new(&obj_file), true) {
            Ok((models, _)) => {
                for model in &models {
                    let model_mesh = &model.mesh;
                    mesh.bounds = BoundingBox::new(&model_mesh.positions[0..3].try_into()?);
                    for face in (0..model_mesh.num_face_indices.len()*3).step_by(3) {
                        let mut vertices: [[f32; 3]; 3] = [[0.0; 3]; 3];
                        let mut normal: [f32; 3] = [0.0; 3];
                        let mut coord = 0;
                        for idx in model_mesh.indices[face..face+3].iter() {
                            let i = 3 * (*idx as usize);
                            if model_mesh.normals.len() > 0 {
                                normal = model_mesh.normals[i..i+3].try_into()?;
                            }
                            vertices[coord] = model_mesh.positions[i..i+3].try_into()?;
                            coord+=1;
                        }
                        let tri = stl_io::Triangle{vertices, normal} ;
                        mesh.process_tri(&tri);
                    }
                }
            }
            Err(e) => panic!("Loading of {:?} failed due to {:?}", obj_file, e),
        }
        if !mesh.stl_had_normals {
            warn!("OBJ file missing surface normals");
        }
        info!("Bounds:");
        info!("{}", mesh.bounds);
        info!("Center:\t{:?}", mesh.bounds.center());

        Ok(mesh)
    }
    pub fn from_stl(mut stl_file: File) -> Result<Mesh, Box<Error>> {
        //let stl = stl_io::read_stl(&mut stl_file)?;
        //debug!("{:?}", stl);
        let mut stl_iter = stl_io::create_stl_reader(&mut stl_file)?;

        // Get starting point for finding bounding box
        let t1 = stl_iter.next().unwrap().unwrap();
        let v1 = t1.vertices[0];

        let mut mesh = Mesh {
            vertices: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
            bounds: BoundingBox::new(&v1),
            stl_had_normals: true,
            obj: false,
        };

        let mut face_count = 0;
        mesh.process_tri(&t1);
        face_count += 1;

        for triangle in stl_iter {
            mesh.process_tri(&triangle?);
            face_count += 1;
            //debug!("{:?}",triangle);
        }

        if !mesh.stl_had_normals {
            warn!("STL file missing surface normals");
        }
        info!("Bounds:");
        info!("{}", mesh.bounds);
        info!("Center:\t{:?}", mesh.bounds.center());
        info!("Triangles processed:\t{}\n", face_count);

        Ok(mesh)
    }

    fn process_tri(&mut self, tri: &stl_io::Triangle) {
        for v in tri.vertices.iter() {
            self.bounds.expand(&v);
            self.vertices.push(Vertex { position: *v });
            //debug!("{:?}", v);
        }
        // Use normal from STL file if it is provided, otherwise calculate it ourselves
        let n: stl_io::Normal;
        if tri.normal == [0.0, 0.0, 0.0] {
            self.stl_had_normals = false;
            n = normal(&tri);
        } else {
            n = tri.normal;
        }
        //debug!("{:?}",tri.normal);
        // TODO: Figure out how to get away with 1 normal instead of 3
        for _ in 0..3 {
            self.normals.push(Normal { normal: n });
        }
    }

    // Move the mesh to be centered at the origin
    // and scaled to fit a 2 x 2 x 2 box. This means that
    // all coordinates will be between -1.0 and 1.0
    pub fn scale_and_center(&self) -> cgmath::Matrix4<f32> {
        // Move center to origin
        let center = self.bounds.center();
        let translation_vector = cgmath::Vector3::new(-center.x, -center.y, -center.z);
        let translation_matrix = cgmath::Matrix4::from_translation(translation_vector);
        // Scale
        let longest = self
            .bounds
            .length()
            .max(self.bounds.width())
            .max(self.bounds.height());
        let scale = 2.0 / longest;
        info!("Scale:\t{}", scale);
        let scale_matrix = cgmath::Matrix4::from_scale(scale);
        let mut matrix = scale_matrix * translation_matrix;
        if self.obj {
            matrix = matrix * cgmath::Matrix4::from_angle_x(cgmath::Rad(3.14/2.0));
        }
        matrix
    }
}

impl fmt::Display for Mesh {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Verts: {}", self.vertices.len())?;
        writeln!(f, "Norms: {}", self.normals.len())?;
        //writeln!(f, "Tex Coords: {:?}", geometry.tex_coords)?;
        writeln!(f, "Indices: {:?}", self.indices.len())?;
        writeln!(f)?;
        Ok(())
    }
}

// Calculate surface normal of triangle using cross product
// TODO: The GPU can probably do this a lot faster than we can.
// See if there is an option for offloading this.
// Probably need to use a geometry shader (not supported in Opengl ES).
fn normal(tri: &stl_io::Triangle) -> stl_io::Normal {
    let p1: cgmath::Vector3<f32> = tri.vertices[0].into();
    let p2: cgmath::Vector3<f32> = tri.vertices[1].into();
    let p3: cgmath::Vector3<f32> = tri.vertices[2].into();
    let v = p2 - p1;
    let w = p3 - p1;
    let n = v.cross(w);
    let mag = n.x.abs() + n.y.abs() + n.z.abs();
    [n.x / mag, n.y / mag, n.z / mag]
}
