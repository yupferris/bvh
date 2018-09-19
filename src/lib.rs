extern crate pest;
#[macro_use]
extern crate pest_derive;

use pest::Parser;
use pest::iterators::Pairs;

use std::io::{self, Write};

#[cfg(debug_assertions)]
const _GRAMMAR: &'static str = include_str!("bvh.pest");

#[derive(Parser)]
#[grammar = "bvh.pest"]
struct BvhParser;

#[derive(Debug)]
pub struct Bvh {
    pub hierarchy: Hierarchy,
    pub motion: Motion,
}

#[derive(Debug)]
pub struct Hierarchy {
    pub root: Joint,
}

#[derive(Debug)]
pub struct Joint {
    pub name: String,
    pub offset: Offset,
    pub channels: Vec<Channel>,
    pub children: JointChildren,
}

impl Joint {
    pub fn total_channels(&self) -> u32 {
        (self.channels.len() as u32) + match self.children {
            JointChildren::Joints(ref joints) => joints.iter().map(|joint| joint.total_channels()).sum(),
            JointChildren::EndSite(_) => 0,
        }
    }
}

#[derive(Debug)]
pub struct Offset {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug)]
pub enum Channel {
    XPosition,
    YPosition,
    ZPosition,
    XRotation,
    YRotation,
    ZRotation,
}

#[derive(Debug)]
pub enum JointChildren {
    Joints(Vec<Joint>),
    EndSite(EndSite),
}

#[derive(Debug)]
pub struct EndSite {
    pub offset: Offset,
}

#[derive(Debug)]
pub struct Motion {
    pub num_frames: u32,
    pub frame_time: f64,
    pub frame_data: Vec<f64>,
}

pub fn parse(input: &str) -> Result<Bvh, String> {
    let mut pairs = BvhParser::parse(Rule::bvh, &input).map_err(|e| format!("Couldn't parse BVH: {:?}", e))?;

    let mut bvh_pairs = pairs.find(|pair| pair.as_rule() == Rule::bvh).unwrap().into_inner();

    let mut hierarchy_pairs = bvh_pairs.find(|pair| pair.as_rule() == Rule::hierarchy).unwrap().into_inner();
    let mut root_pairs = hierarchy_pairs.find(|pair| pair.as_rule() == Rule::root_joint).unwrap().into_inner();
    let root_joint_body_pairs = root_pairs.find(|pair| pair.as_rule() == Rule::joint_body).unwrap().into_inner();
    let root = parse_joint(root_joint_body_pairs);

    let mut motion_pairs = bvh_pairs.find(|pair| pair.as_rule() == Rule::motion).unwrap().into_inner();
    let mut frames_pairs = motion_pairs.find(|pair| pair.as_rule() == Rule::frames).unwrap().into_inner();
    let motion = Motion {
        num_frames: frames_pairs.find(|pair| pair.as_rule() == Rule::integer).unwrap().as_str().parse::<u32>().unwrap(),
        frame_time: parse_f64(&mut frames_pairs),
        frame_data: frames_pairs.filter(|pair| pair.as_rule() == Rule::float).map(|pair| pair.as_str().parse::<f64>().unwrap()).collect(),
    };

    Ok(Bvh {
        hierarchy: Hierarchy {
            root: root,
        },
        motion: motion,
    })
}

fn parse_joint(mut joint_body_pairs: Pairs<Rule>) -> Joint {
    let name = joint_body_pairs.find(|pair| pair.as_rule() == Rule::identifier).unwrap().as_str().into();
    let mut offset_pairs = joint_body_pairs.find(|pair| pair.as_rule() == Rule::offset).unwrap().into_inner();
    let offset = parse_offset(&mut offset_pairs);
    let channel_pairs = joint_body_pairs.find(|pair| pair.as_rule() == Rule::channels).unwrap().into_inner();
    let joints: Vec<Joint> = joint_body_pairs.clone().filter(|pair| pair.as_rule() == Rule::joint).map(|pair| {
        let body_pairs = pair.into_inner().find(|pair| pair.as_rule() == Rule::joint_body).unwrap().into_inner();
        parse_joint(body_pairs)
    }).collect();
    let children = if joints.len() > 0 {
        JointChildren::Joints(joints)
    } else {
        let mut end_site_pairs = joint_body_pairs.find(|pair| pair.as_rule() == Rule::end_site).unwrap().into_inner();
        let mut offset_pairs = end_site_pairs.find(|pair| pair.as_rule() == Rule::offset).unwrap().into_inner();
        let offset = parse_offset(&mut offset_pairs);
        JointChildren::EndSite(EndSite {
            offset: offset,
        })
    };
    Joint {
        name: name,
        offset: offset,
        channels: channel_pairs.filter(|pair| pair.as_rule() == Rule::channel).map(|pair| match pair.as_str() {
            "Xposition" => Channel::XPosition,
            "Yposition" => Channel::YPosition,
            "Zposition" => Channel::ZPosition,
            "Xrotation" => Channel::XRotation,
            "Yrotation" => Channel::YRotation,
            "Zrotation" => Channel::ZRotation,
            _ => unreachable!()
        }).collect(),
        children: children,
    }
}

fn parse_offset(offset_pairs: &mut Pairs<Rule>) -> Offset {
    Offset {
        x: parse_f64(offset_pairs),
        y: parse_f64(offset_pairs),
        z: parse_f64(offset_pairs),
    }
}

fn parse_f64(offset_pairs: &mut Pairs<Rule>) -> f64 {
    offset_pairs.find(|pair| pair.as_rule() == Rule::float).unwrap().as_str().parse::<f64>().unwrap()
}

pub fn serialize<W: Write>(bvh: &Bvh, w: &mut W) -> io::Result<()> {
    serialize_hierarchy(&bvh.hierarchy, w)?;
    serialize_motion(&bvh.motion, bvh.hierarchy.root.total_channels(), w)?;

    Ok(())
}

fn serialize_hierarchy<W: Write>(hierarchy: &Hierarchy, w: &mut W) -> io::Result<()> {
    writeln!(w, "HIERARCHY")?;
    write!(w, "ROOT ")?;
    serialize_joint(&hierarchy.root, w)?;

    Ok(())
}

fn serialize_joint<W: Write>(joint: &Joint, w: &mut W) -> io::Result<()> {
    writeln!(w, "{}", joint.name)?;
    writeln!(w, "{{")?;

    serialize_offset(&joint.offset, w)?;

    write!(w, "CHANNELS {}", joint.channels.len())?;
    for channel in joint.channels.iter() {
        match *channel {
            Channel::XPosition => write!(w, " Xposition"),
            Channel::YPosition => write!(w, " Yposition"),
            Channel::ZPosition => write!(w, " Zposition"),
            Channel::XRotation => write!(w, " Xrotation"),
            Channel::YRotation => write!(w, " Yrotation"),
            Channel::ZRotation => write!(w, " Zrotation"),
        }?;
    }
    writeln!(w, "")?;

    match joint.children {
        JointChildren::Joints(ref joints) => {
            for joint in joints.iter() {
                write!(w, "JOINT ")?;
                serialize_joint(joint, w)?;
            }
        }
        JointChildren::EndSite(ref end_site) => {
            writeln!(w, "End Site")?;
            writeln!(w, "{{")?;

            serialize_offset(&end_site.offset, w)?;

            writeln!(w, "}}")?;
        }
    }

    writeln!(w, "}}")?;

    Ok(())
}

fn serialize_offset<W: Write>(offset: &Offset, w: &mut W) -> io::Result<()> {
    writeln!(w, "OFFSET {} {} {}", offset.x, offset.y, offset.z)
}

fn serialize_motion<W: Write>(motion: &Motion, total_channels: u32, w: &mut W) -> io::Result<()> {
    writeln!(w, "MOTION")?;
    writeln!(w, "Frames: {}", motion.num_frames)?;
    writeln!(w, "Frame Time: {}", motion.frame_time)?;

    for (index, value) in motion.frame_data.iter().enumerate() {
        write!(w, "{}", value)?;
        if (index as u32) % total_channels != total_channels - 1 {
            write!(w, " ")?;
        } else {
            writeln!(w, "")?;
        }
    }

    Ok(())
}
