use anyhow::{format_err, Error};
use libxml::parser::Parser;
use libxml::tree::{self, Document, NodeType};
use libxml::xpath::Context;
use std::{fmt, ops::Deref, rc::Rc};

#[derive(Debug)]
pub enum Value {
    Element(Vec<Node>),
    Text(Vec<String>),
    None,
}

impl Value {
    pub fn into_element(self) -> Option<Vec<Node>> {
        match self {
            Value::Element(v) => Some(v),
            _ => None,
        }
    }

    pub fn into_text(self) -> Option<Vec<String>> {
        match self {
            Value::Text(v) => Some(v),
            _ => None,
        }
    }
}

pub struct Node {
    document: Rc<Document>,
    context: Rc<Context>,
    node: tree::Node,
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.get_type() {
            Some(NodeType::ElementNode) => {
                write!(f, "<Element {} at {:p}>", self.get_name(), self.node_ptr())
            }
            Some(NodeType::AttributeNode) | Some(NodeType::TextNode) => {
                write!(f, "{:?}", self.get_content())
            }
            _ => unimplemented!(),
        }
    }
}

impl Node {
    pub fn xpath_text(&self, xpath: &str) -> Result<Vec<String>, Error> {
        match self.xpath(xpath)?.into_text() {
            Some(v) => Ok(v),
            None => Err(format_err!("not found")),
        }
    }

    pub fn xpath_elem(&self, xpath: &str) -> Result<Vec<Node>, Error> {
        match self.xpath(xpath)?.into_element() {
            Some(v) => Ok(v),
            None => Err(format_err!("not found")),
        }
    }

    pub fn xpath(&self, xpath: &str) -> Result<Value, Error> {
        let nodes = self
            .context
            .node_evaluate(xpath, &self.node)
            .map_err(|_| format_err!("failed to evaluate xpath"))?
            .get_nodes_as_vec();
        let result = match nodes.get(0) {
            Some(node) => match node.get_type() {
                Some(NodeType::ElementNode) => Value::Element(
                    nodes
                        .into_iter()
                        .map(|node| Node {
                            document: self.document.clone(),
                            context: self.context.clone(),
                            node,
                        })
                        .collect(),
                ),
                Some(NodeType::AttributeNode) | Some(NodeType::TextNode) => {
                    Value::Text(nodes.into_iter().map(|node| node.get_content()).collect())
                }
                _ => unimplemented!(),
            },
            None => Value::None,
        };
        Ok(result)
    }
}

impl Deref for Node {
    type Target = tree::Node;

    fn deref(&self) -> &Self::Target {
        &self.node
    }
}

pub fn parse_html<S: AsRef<str>>(html: S) -> Result<Node, Error> {
    let parser = Parser::default_html();
    let document = parser
        .parse_string(html.as_ref())
        .map_err(|_| format_err!("failed to parse html"))?;
    let context = Context::new(&document).map_err(|_| format_err!("failed to new context"))?;
    let root = document.get_root_element().expect("no root element");
    Ok(Node {
        document: Rc::new(document),
        context: Rc::new(context),
        node: root,
    })
}

#[cfg(test)]
mod tests {
    use crate::xpath::parse_html;

    #[test]
    fn find_nodes() {
        let html = r#"
        <!doctype html>
        <html lang="zh-CN" dir="ltr">
          <head>
            <meta charset="utf-8">
            <meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline' resource: chrome:; connect-src https:; img-src https: data: blob:; style-src 'unsafe-inline';">
            <title>新标签页</title>
            <link rel="icon" type="image/png" href="chrome://branding/content/icon32.png"/>
            <link rel="stylesheet" href="chrome://browser/content/contentSearchUI.css" />
            <link rel="stylesheet" href="resource://activity-stream/css/activity-stream.css" />
          </head>
          <body class="activity-stream">
            <div id="root"><!-- Regular React Rendering --></div>
            <div id="snippets-container">
              <div id="snippets"></div>
            </div>
             <table id="wow" class="lol">
              <tr class="head">
                <th>Firstname</th>
                <th>Lastname</th>
                <th>Age</th>
              </tr>
              <tr class="body">
                <td>Jill</td>
                <td>Smith</td>
                <td>50</td>
              </tr>
              <tr class="body">
                <td>Eve</td>
                <td>Jackson</td>
                <td>94</td>
              </tr>
             </table>
          </body>
        </html>
        "#;
        let node = parse_html(html).unwrap();
        println!("{:?}", node.xpath(r#"//table"#));
        println!("{:?}", node.xpath(r#"//table/@class"#));
        println!("{:?}", node.xpath(r#"//table//tr"#));
        println!("{:?}", node.xpath(r#"//table//th/text()"#));

        for td in node.xpath("//td").unwrap().into_element().unwrap() {
            println!("{:?}", td.xpath(".//text()"));
        }
    }
}
