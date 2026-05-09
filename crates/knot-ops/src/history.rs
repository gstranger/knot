use std::sync::Arc;
use knot_core::Id;
use knot_topo::BRep;
use super::BooleanOp;

/// A node in the parametric construction history.
/// The tree is append-only and immutable.
#[derive(Clone, Debug)]
pub enum OpNode {
    Primitive {
        id: Id<OpNode>,
        kind: PrimitiveKind,
        result: Arc<BRep>,
    },
    Boolean {
        id: Id<OpNode>,
        op: BooleanOp,
        left: Arc<OpNode>,
        right: Arc<OpNode>,
        result: Arc<BRep>,
    },
    UnaryOp {
        id: Id<OpNode>,
        kind: UnaryOpKind,
        input: Arc<OpNode>,
        result: Arc<BRep>,
    },
}

impl OpNode {
    pub fn result(&self) -> &Arc<BRep> {
        match self {
            OpNode::Primitive { result, .. } => result,
            OpNode::Boolean { result, .. } => result,
            OpNode::UnaryOp { result, .. } => result,
        }
    }

    pub fn id(&self) -> Id<OpNode> {
        match self {
            OpNode::Primitive { id, .. } => *id,
            OpNode::Boolean { id, .. } => *id,
            OpNode::UnaryOp { id, .. } => *id,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimitiveKind {
    Box,
    Sphere,
    Cylinder,
    Cone,
    Torus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOpKind {
    Extrude,
    Revolve,
    Sweep,
    Fillet,
    Chamfer,
}
