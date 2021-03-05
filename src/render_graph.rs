use std::{borrow::Cow, convert::TryInto};

use crate::{
    dot::{font_tag, DotGraph},
    utils,
};
use bevy::render::render_graph::{NodeId, RenderGraph};

use itertools::{EitherOrBoth, Itertools};
use tabbycat::{
    attributes::*, AttrList, AttrType, Compass, Edge, GraphBuilder, GraphType, Identity, Port,
    Stmt, StmtList, TabbyCatError,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DebugDumpError {
    #[error(transparent)]
    TabbyCatError(#[from] tabbycat::TabbyCatError),
    #[error("Failed to build graph: {0}")]
    GraphBuilderError(String),
}

pub fn render_graph_dot(graph: &RenderGraph) -> Result<String, DebugDumpError> {
    let dot = to_dot(graph)?;
    Ok(format!("{}", dot))
}

pub fn to_dot(render_graph: &RenderGraph) -> Result<tabbycat::Graph, DebugDumpError> {
    // Iterator over results to result with iterator adapted from https://stackoverflow.com/a/63120052/1152077
    let mut err: Result<(), DebugDumpError> = Result::Ok(());

    let graph = GraphBuilder::default()
        .graph_type(GraphType::DiGraph)
        .strict(false)
        .id(Identity::id("RenderGraph").unwrap())
        .stmts(
            StmtList::new()
                .add_attr(
                    AttrType::Node,
                    AttrList::new()
                        .add("shape".try_into()?, "plaintext".try_into()?)
                        .add("fontname".try_into()?, "Roboto".try_into()?),
                )
                .add_attr(
                    AttrType::Graph,
                    AttrList::new().add("rankdir".try_into()?, "LR".try_into()?),
                )
                .extend(
                    map_nodes(render_graph).scan(&mut err, |err, res| match res {
                        Ok(o) => Some(o),
                        Err(e) => {
                            **err = Err(e);
                            None
                        }
                    }),
                )
                .extend(map_edges(render_graph)?),
        )
        .build()
        .map_err(DebugDumpError::GraphBuilderError)?;

    err?;

    Ok(graph)
}

// TODO figure out how to work around the borrowchecker to return Result<Iterator<Stmt>> instead
fn map_nodes(render_graph: &RenderGraph) -> impl Iterator<Item = Result<Stmt, DebugDumpError>> {
    // TODO sort nodes
    // let mut nodes: Vec<_> = graph.iter_nodes().collect();
    // nodes.sort_by_key(|node_state| &node_state.type_name);

    render_graph
    .iter_nodes()
    .map(|node| {
        let name = node.name.as_deref().unwrap_or("<node>");
        let id = node.id.uuid().as_u128().into();
        Ok(Stmt::Node {
            id,
            port: None,
            attr: Some(AttrList::new().add(
                "label".try_into()?,
                Identity::raw(format!(
                    "<<TABLE><TR><TD PORT=\"title\" BORDER=\"0\" COLSPAN=\"2\">{}<BR/>{}<BR/><FONT COLOR=\"red\" POINT-SIZE=\"10\">{}</FONT></TD></TR>{}</TABLE>>",
                    escape_html(name),
                    // TODO make optional
                    escape_html(format!("{}", node.id.uuid())),
                    // TODO use TypeRegistry
                    escape_html(utils::short_name(node.type_name)),
                    node.output_slots.iter().enumerate().zip_longest(node.input_slots.iter().enumerate()).map(|pair| {
                        match pair {
                            EitherOrBoth::Both(input, output) =>format!("<TR><TD PORT=\"{}\">{}: {}</TD><TD PORT=\"{}\">{}: {}</TD></TR>", input.0, escape_html(input.1.info.name.as_ref()), escape_html(format!("{:?}", input.1.info.resource_type)), output.0, escape_html(output.1.info.name.as_ref()), escape_html(format!("{:?}", output.1.info.resource_type))),
                            EitherOrBoth::Left(input) =>format!("<TR><TD PORT=\"{}\">{}: {:?}</TD><TD BORDER=\"0\">&nbsp;</TD></TR>", input.0, input.1.info.name, input.1.info.resource_type),
                            EitherOrBoth::Right(output) =>format!("<TR><TD BORDER=\"0\">&nbsp;</TD><TD PORT=\"{}\">{}: {:?}</TD></TR>", output.0, output.1.info.name, output.1.info.resource_type)
                        }
                    }).collect::<String>()
                )),
            )),
        })
    })
}

fn map_edges(render_graph: &RenderGraph) -> Result<impl Iterator<Item = Stmt>, TabbyCatError> {
    let edges = render_graph.iter_nodes().flat_map(|node| {
        node.edges.input_edges.iter().map(|edge| match edge {
            bevy::render::render_graph::Edge::SlotEdge {
                input_node,
                input_index,
                output_node,
                output_index,
            } => Stmt::Edge(
                Edge::head_node(
                    input_node.uuid().as_u128().into(),
                    Some(Port::id_compass((*input_index).into(), Compass::East)),
                )
                .arrow_to_node(
                    output_node.uuid().as_u128().into(),
                    Some(Port::id_compass((*output_index).into(), Compass::West)),
                ),
            ),
            bevy::render::render_graph::Edge::NodeEdge {
                input_node,
                output_node,
            } => Stmt::Edge(
                Edge::head_node(
                    output_node.uuid().as_u128().into(),
                    Some(Port::id_compass(Identity::raw("title"), Compass::East)),
                )
                .arrow_to_node(
                    input_node.uuid().as_u128().into(),
                    Some(Port::id_compass(Identity::raw("title"), Compass::West)),
                )
                .add_attrpair(tabbycat::attributes::style(Style::Dashed)),
            ),
        })
    });
    Ok(edges)
}

/// Escape tags in such a way that it is suitable for inclusion in a
/// Graphviz HTML label.
pub fn escape_html<'a, S>(s: S) -> Cow<'a, str>
where
    S: Into<Cow<'a, str>>,
{
    s.into()
        .replace("&", "&amp;")
        .replace("\"", "&quot;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .into()
}
