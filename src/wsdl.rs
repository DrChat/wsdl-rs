use roxmltree::{Document, ExpandedName, Node, NodeId};
use thiserror::Error;

type Result<'a, 'input, T> = std::result::Result<T, WsError>;

#[derive(Error, Debug)]
pub enum WsErrorMalformedType {
    #[error("missing attribute \"{0}\"")]
    MissingAttribute(String),
    #[error("missing element \"{0}\"")]
    MissingElement(String),
}

#[derive(Error, Debug)]
pub enum WsErrorType {
    #[error("The input WSDL document was malformed: {0}")]
    MalformedWsdl(WsErrorMalformedType),
    #[error("Attempt to refer to unknown element {0}")]
    InvalidReference(String),
    #[error("Node unexpectedly did not have a parent node")]
    NoParentNode,
}

#[derive(Error, Debug)]
pub struct WsError(pub NodeId, pub WsErrorType);

impl WsError {
    fn new(node: Node, typ: WsErrorType) -> Self {
        Self(node.id(), typ)
    }
}

impl std::fmt::Display for WsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.1))
    }
}

fn target_namespace<'a, 'input>(node: Node<'a, 'input>) -> Result<'a, 'input, &'a str> {
    // Traverse the parents until we find the targetNamespace attribute.
    let mut nparent = node.parent();
    while let Some(parent) = nparent {
        if let Some(ns) = parent.attribute("targetNamespace") {
            return Ok(ns);
        }

        nparent = parent.parent();
    }

    Err(WsError::new(
        node,
        WsErrorType::MalformedWsdl(WsErrorMalformedType::MissingAttribute(
            "targetNamespace".to_string(),
        )),
    ))
}

fn resolve_qualified<'a, 'input: 'a>(
    node: Node<'a, 'input>,
    qualified_name: &'a str,
) -> std::result::Result<ExpandedName<'a, 'a>, WsErrorType> {
    if qualified_name.contains(":") {
        let mut s = qualified_name.split(":");

        let ns = s
            .next()
            .ok_or(WsErrorType::InvalidReference(qualified_name.to_string()))?;

        let uri = node
            .lookup_namespace_uri(Some(ns))
            .ok_or(WsErrorType::InvalidReference(qualified_name.to_string()))?;

        let name = s
            .next()
            .ok_or(WsErrorType::InvalidReference(qualified_name.to_string()))?;

        Ok((uri, name).into())
    } else {
        Ok(qualified_name.into())
    }
}

fn split_qualified(qualified_name: &str) -> std::result::Result<(Option<&str>, &str), WsErrorType> {
    let (namespace, name) = {
        if qualified_name.contains(":") {
            let mut s = qualified_name.split(":");
            let ns = s
                .next()
                .ok_or(WsErrorType::InvalidReference(qualified_name.to_string()))?;

            let name = s
                .next()
                .ok_or(WsErrorType::InvalidReference(qualified_name.to_string()))?;

            (Some(ns), name)
        } else {
            (None, qualified_name)
        }
    };

    Ok((namespace, name))
}

// Given a qualified name such as `tns:MyAnnoyingXmlType`, look for an XML
// node with both the name and element type.
/*
fn lookup_qualified<'a, 'input>(
    root: Node<'a, 'input>,
    name: &str,
    tag: &str,
) -> Option<Node<'a, 'input>> {
    todo!()
}
*/

/// Describes a WSDL `message`. These can otherwise be described as
/// a list of function parameters.
#[derive(Debug, Clone)]
pub struct WsMessage<'a, 'input>(Node<'a, 'input>);

impl<'a, 'input> WsMessage<'a, 'input> {
    /// Retrieve the name of the message.
    pub fn name(&self) -> Result<&'a str> {
        self.0.attribute("name").ok_or(WsError::new(
            self.0,
            WsErrorType::MalformedWsdl(WsErrorMalformedType::MissingAttribute("name".to_string())),
        ))
    }

    /// Retrieve the parts of this message.
    pub fn parts(&self) -> impl Iterator<Item = WsMessagePart> {
        self.0
            .children()
            .filter(|n| n.has_tag_name(("http://schemas.xmlsoap.org/wsdl/", "part")))
            .map(|n| WsMessagePart(n))
    }

    /// Return the XML node this struct is associated with
    pub fn node(&self) -> Node<'a, 'input> {
        self.0
    }
}

/// Describes a part of a WSDL message. This can otherwise be described
/// as an individual function parameter.
#[derive(Debug, Clone)]
pub struct WsMessagePart<'a, 'input>(Node<'a, 'input>);

impl<'a, 'input: 'a> WsMessagePart<'a, 'input> {
    /// Retrieve the name of the part.
    pub fn name(&self) -> Result<&'a str> {
        self.0.attribute("name").ok_or(WsError::new(
            self.0,
            WsErrorType::MalformedWsdl(WsErrorMalformedType::MissingAttribute("name".to_string())),
        ))
    }

    /// Retrieve the typename of this parameter. This refers to a type defined
    /// under the `wsdl:types` XML node.
    pub fn typename(&self) -> Result<ExpandedName<'a, 'a>> {
        let typename = self
            .0
            .attribute("element")
            .or(self.0.attribute("type"))
            .ok_or(WsError::new(
                self.0,
                WsErrorType::MalformedWsdl(WsErrorMalformedType::MissingAttribute(
                    "type".to_string(),
                )),
            ))?;

        resolve_qualified(self.0, typename).map_err(|e| WsError::new(self.0, e))
    }

    /// Return the XML node this struct is associated with
    pub fn node(&self) -> Node<'a, 'input> {
        self.0
    }
}

/// Describes a WSDL `portType`. These describe groups of operations.
#[derive(Debug, Clone)]
pub struct WsPortType<'a, 'input>(Node<'a, 'input>);

impl<'a, 'input> WsPortType<'a, 'input> {
    /// Retrieve the name of the port type.
    pub fn name(&self) -> Result<&'a str> {
        self.0.attribute("name").ok_or(WsError::new(
            self.0,
            WsErrorType::MalformedWsdl(WsErrorMalformedType::MissingAttribute("name".to_string())),
        ))
    }

    /// Retrieve the port type's target namespace.
    pub fn target_namespace(&self) -> Result<&'a str> {
        target_namespace(self.0)
    }

    /// Retrieve the operations associated with this port.
    pub fn operations(&self) -> Result<impl Iterator<Item = WsPortOperation<'a, 'input>>> {
        Ok(self
            .0
            .children()
            .filter(|n| n.has_tag_name(("http://schemas.xmlsoap.org/wsdl/", "operation")))
            .map(|n| WsPortOperation(n)))
    }

    /// Return the XML node this struct is associated with
    pub fn node(&self) -> Node<'a, 'input> {
        self.0
    }
}

/// Describes an operation associated with a WSDL `portType`.
/// A WSDL operation can otherwise be described as a function.
#[derive(Debug, Clone)]
pub struct WsPortOperation<'a, 'input>(Node<'a, 'input>);

impl<'a, 'input> WsPortOperation<'a, 'input> {
    /// Retrieve the name of an operation.
    pub fn name(&self) -> Result<&'a str> {
        self.0.attribute("name").ok_or(WsError::new(
            self.0,
            WsErrorType::MalformedWsdl(WsErrorMalformedType::MissingAttribute("name".to_string())),
        ))
    }

    /// Retrieve the input message for this port.
    pub fn input(&self) -> Result<Option<WsMessage<'a, 'input>>> {
        let message_typename = match self
            .0
            .children()
            .find(|n| n.has_tag_name(("http://schemas.xmlsoap.org/wsdl/", "input")))
            .map(|n| n.attribute("message"))
            .flatten()
        {
            Some(n) => n,
            None => return Ok(None),
        };

        let (_message_namespace, message_name) =
            split_qualified(message_typename).map_err(|e| WsError::new(self.0, e))?;

        let def = WsDefinitions::find_parent(self.0)?;
        Ok(Some(
            def.messages()?
                .find(|n| n.0.attribute("name") == Some(message_name))
                .ok_or(WsError::new(
                    self.0,
                    WsErrorType::InvalidReference(message_name.to_string()),
                ))?,
        ))
    }

    /// Retrieve the output message for this port.
    pub fn output(&self) -> Result<Option<WsMessage<'a, 'input>>> {
        let mut outputs = self.outputs()?;
        let output = outputs.next();
        if outputs.next().is_some() {
            panic!("Multiple output messages found for operation {:?}", self.name()?);
        }
         Ok(output)
    }

    /// Retrieve all output messages for this port
    pub fn outputs(&self) -> Result<impl Iterator<Item=WsMessage<'a, 'input>>> {
        let def = WsDefinitions::find_parent(self.0)?;
        Ok(self
           .0
           .children()
           .filter(|n| n.has_tag_name(("http://schemas.xmlsoap.org/wsdl/", "output")))
           .filter_map(|n| Some((n.attribute("name")?, n.attribute("message")?)))
           .filter_map(|(name, message_typename)| match split_qualified(message_typename) {
               Ok((_message_namespace, message_name)) => Some((name, message_name)),
               Err(_) => None
           }).filter_map(move |(name, message_name)| {
            let Ok(mut messages) = def.messages() else {
                return None;
            };
            if let Some(message) = messages.find(|n| n.0.attribute("name") == Some(message_name)) {
                Some(message)
            } else {
                None
            }
        }))
    }

    /// Retrieve the first fault message for this port.
    pub fn fault(&self) -> Result<Option<WsMessage<'a, 'input>>> {
        let mut faults = self.faults()?;
        let fault = faults.next();
        if faults.next().is_some() {
            panic!("Multiple fault messages found for operation {:?}", self.name()?);
        }
        Ok(fault)
    }

    /// Retrieve all fault messages for this port
    pub fn faults(&self) -> Result<impl Iterator<Item=WsMessage<'a, 'input>>> {
        let def = WsDefinitions::find_parent(self.0)?;
        Ok(self
           .0
           .children()
           .filter(|n| n.has_tag_name(("http://schemas.xmlsoap.org/wsdl/", "fault")))
           .filter_map(|n| Some((n.attribute("name")?, n.attribute("message")?)))
           .filter_map(|(name, message_typename)| match split_qualified(message_typename) {
               Ok((_message_namespace, message_name)) => Some((name, message_name)),
               Err(_) => None
           }).filter_map(move |(name, message_name)| {
            let Ok(mut messages) = def.messages() else {
                return None;
            };
            if let Some(message) = messages.find(|n| n.0.attribute("name") == Some(message_name)) {
                Some(message)
            } else {
                None
            }
        }))
    }

    /// Return the XML node this struct is associated with
    pub fn node(&self) -> Node<'a, 'input> {
        self.0
    }
}

/// A WSDL binding operation.
#[derive(Debug, Clone)]
pub struct WsBindingOperation<'a, 'input>(Node<'a, 'input>);

impl<'a, 'input> WsBindingOperation<'a, 'input> {
    /// Return the name of the operation described.
    pub fn name(&self) -> Result<&'a str> {
        self.0.attribute("name").ok_or(WsError::new(
            self.0,
            WsErrorType::MalformedWsdl(WsErrorMalformedType::MissingAttribute("name".to_string())),
        ))
    }

    /// Retrieve the port operation that corresponds to this binding operation.
    pub fn port_operation(&self) -> Result<WsPortOperation<'a, 'input>> {
        let name = self.name()?;
        let binding = WsBinding(
            self.0
                .parent()
                .ok_or(WsError::new(self.0, WsErrorType::NoParentNode))?,
        );

        let port_type: WsPortType<'a, 'input> = binding.port_type()?;

        for op in port_type.operations()? {
            if op.name()? == name {
                return Ok(op);
            }
        }
        Err(WsError::new(
            self.0,
            WsErrorType::MalformedWsdl(WsErrorMalformedType::MissingElement(name.to_string())),
        ))
    }

    /// Return the XML node this struct is associated with
    pub fn node(&self) -> Node<'a, 'input> {
        self.0
    }
}

/// A WSDL binding that describes how the operations in a port type
/// are bound to/from the wire.
#[derive(Debug, Clone)]
pub struct WsBinding<'a, 'input>(Node<'a, 'input>);

impl<'a, 'input> WsBinding<'a, 'input> {
    /// Retrieve the name of a binding.
    pub fn name(&self) -> Result<&'a str> {
        self.0.attribute("name").ok_or(WsError::new(
            self.0,
            WsErrorType::MalformedWsdl(WsErrorMalformedType::MissingAttribute("name".to_string())),
        ))
    }

    pub fn port_type(&self) -> Result<WsPortType<'a, 'input>> {
        let port_typename = self.0.attribute("type").ok_or(WsError::new(
            self.0,
            WsErrorType::MalformedWsdl(WsErrorMalformedType::MissingAttribute("type".to_string())),
        ))?;

        let (_port_namespace, port_name) =
            split_qualified(port_typename).map_err(|e| WsError::new(self.0, e))?;

        let def = WsDefinitions::find_parent(self.0)?;
        def.port_types()?
            .find(|n| n.0.attribute("name") == Some(port_name))
            .ok_or(WsError::new(
                self.0,
                WsErrorType::InvalidReference(port_name.to_string()),
            ))
    }

    pub fn operations(&self) -> Result<impl Iterator<Item = WsBindingOperation>> {
        Ok(self
            .0
            .children()
            .filter(|n| n.has_tag_name(("http://schemas.xmlsoap.org/wsdl/", "operation")))
            .map(|n| WsBindingOperation(n)))
    }

    /// Return the XML node this struct is associated with
    pub fn node(&self) -> Node<'a, 'input> {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct WsServicePort<'a, 'input>(Node<'a, 'input>);

impl<'a, 'input> WsServicePort<'a, 'input> {
    pub fn name(&self) -> Result<&'a str> {
        self.0.attribute("name").ok_or(WsError::new(
            self.0,
            WsErrorType::MalformedWsdl(WsErrorMalformedType::MissingAttribute("name".to_string())),
        ))
    }

    /// Fetch the binding information associated with this service port.
    pub fn binding(&self) -> Result<WsBinding<'a, 'input>> {
        let binding_typename = self.0.attribute("binding").ok_or(WsError::new(
            self.0,
            WsErrorType::MalformedWsdl(WsErrorMalformedType::MissingAttribute(
                "binding".to_string(),
            )),
        ))?;

        let (_binding_namespace, binding_name) =
            split_qualified(binding_typename).map_err(|e| WsError::new(self.0, e))?;

        let def = WsDefinitions::find_parent(self.0)?;
        def.bindings()?
            .find(|n| n.0.attribute("name") == Some(binding_name))
            .ok_or(WsError::new(
                self.0,
                WsErrorType::InvalidReference(binding_name.to_string()),
            ))
    }

    /// Return the XML node this struct is associated with
    pub fn node(&self) -> Node<'a, 'input> {
        self.0
    }
}

/// A WSDL service, usually describing an HTTP endpoint that serves
/// messages bound with a [WsBinding]
#[derive(Debug, Clone)]
pub struct WsService<'a, 'input>(Node<'a, 'input>);

impl<'a, 'input> WsService<'a, 'input> {
    pub fn name(&self) -> Result<&'a str> {
        self.0.attribute("name").ok_or(WsError::new(
            self.0,
            WsErrorType::MalformedWsdl(WsErrorMalformedType::MissingAttribute("name".to_string())),
        ))
    }

    pub fn ports(&self) -> Result<impl Iterator<Item = WsServicePort>> {
        Ok(self
            .0
            .children()
            .filter(|n| n.has_tag_name(("http://schemas.xmlsoap.org/wsdl/", "port")))
            .map(|n| WsServicePort(n)))
    }

    /// Return the XML node this struct is associated with
    pub fn node(&self) -> Node<'a, 'input> {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct WsTypes<'a, 'input>(Node<'a, 'input>);

impl<'a, 'input> WsTypes<'a, 'input> {
    /// Return the schemas contained within. These are defined according to the XML schema specification,
    /// and are out of scope for this library to interpret.
    pub fn schemas(&self) -> Result<impl Iterator<Item = Node<'a, 'input>>> {
        Ok(self
            .0
            .children()
            .filter(|n| n.has_tag_name(("http://www.w3.org/2001/XMLSchema", "schema"))))
    }
}

#[derive(Debug, Clone)]
pub struct WsDefinitions<'a, 'input>(Node<'a, 'input>);

impl<'a, 'input> WsDefinitions<'a, 'input> {
    /// Find the definitions block from one of the node's parents
    fn find_parent(mut node: Node<'a, 'input>) -> Result<'a, 'input, Self> {
        loop {
            node = node
                .parent()
                .ok_or(WsError::new(node, WsErrorType::NoParentNode))?;

            if let Ok(definitions) = Self::from_node(node) {
                return Ok(definitions);
            }
        }
    }

    pub fn from_node(node: Node<'a, 'input>) -> Result<'a, 'input, Self> {
        if node.has_tag_name(("http://schemas.xmlsoap.org/wsdl/", "definitions")) {
            Ok(Self(node))
        } else {
            Err(WsError::new(
                node,
                WsErrorType::MalformedWsdl(WsErrorMalformedType::MissingElement(
                    "definitions".to_string(),
                )),
            ))
        }
    }

    pub fn from_document(document: &'a Document<'input>) -> Result<'a, 'input, Self> {
        document
            .root()
            .children()
            .find(|n| n.has_tag_name(("http://schemas.xmlsoap.org/wsdl/", "definitions")))
            .ok_or(WsError::new(
                document.root_element(),
                WsErrorType::MalformedWsdl(WsErrorMalformedType::MissingElement(
                    "definitions".to_string(),
                )),
            ))
            .map(|n| Self(n))
    }

    pub fn port_types(&self) -> Result<impl Iterator<Item = WsPortType<'a, 'input>>> {
        Ok(self
            .0
            .children()
            .filter(|n| n.has_tag_name(("http://schemas.xmlsoap.org/wsdl/", "portType")))
            .map(|n| WsPortType(n))
            .into_iter())
    }

    pub fn messages(&self) -> Result<impl Iterator<Item = WsMessage<'a, 'input>>> {
        Ok(self
            .0
            .children()
            .filter(|n| n.has_tag_name(("http://schemas.xmlsoap.org/wsdl/", "message")))
            .map(|n| WsMessage(n))
            .into_iter())
    }

    pub fn bindings(&self) -> Result<impl Iterator<Item = WsBinding<'a, 'input>>> {
        Ok(self
            .0
            .children()
            .filter(|n| n.has_tag_name(("http://schemas.xmlsoap.org/wsdl/", "binding")))
            .map(|n| WsBinding(n))
            .into_iter())
    }

    pub fn services(&self) -> Result<impl Iterator<Item = WsService<'a, 'input>>> {
        Ok(self
            .0
            .children()
            .filter(|n| n.has_tag_name(("http://schemas.xmlsoap.org/wsdl/", "service")))
            .map(|n| WsService(n))
            .into_iter())
    }

    pub fn types(&self) -> Result<impl Iterator<Item = Node<'a, 'input>>> {
        // FIXME: I'm pretty sure only one of these nodes can exist?
        Ok(self
            .0
            .children()
            .filter(|n| n.has_tag_name(("http://schemas.xmlsoap.org/wsdl/", "types")))
            .into_iter())
    }
}
