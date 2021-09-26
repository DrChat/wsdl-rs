use roxmltree::{Document, Node, NodeId};
use thiserror::Error;

type Result<'a, 'input, T> = std::result::Result<T, WsError>;

#[derive(Error, Debug)]
pub enum WsErrorMalformedType {
    #[error("missing attribute \"{0}\"")]
    MissingAttribute(String),
    #[error("missing element \"{0}\"")]
    MissingElement(String),
    #[error("missing XML namespace")]
    MissingNamespace,
}

#[derive(Error, Debug)]
pub enum WsErrorType {
    #[error("The input WSDL document was malformed: {0}")]
    MalformedWsdl(WsErrorMalformedType),
    #[error("Element has unknown namespace {0}")]
    InvalidNamespace(String),
    #[error("Attempt to refer to unknown element {0}")]
    InvalidReference(String),
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
    while let Some(parent) = node.parent() {
        if let Some(ns) = parent.attribute("targetNamespace") {
            return Ok(ns);
        }
    }

    Err(WsError::new(
        node,
        WsErrorType::MalformedWsdl(WsErrorMalformedType::MissingAttribute(
            "targetNamespace".to_string(),
        )),
    ))
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
            .filter(|n| n.has_tag_name("part"))
            .map(|n| WsMessagePart(n))
    }
}

/// Describes a part of a WSDL message. This can otherwise be described
/// as an individual function parameter.
pub struct WsMessagePart<'a, 'input>(Node<'a, 'input>);

impl<'a, 'input> WsMessagePart<'a, 'input> {
    /// Retrieve the name of the part.
    pub fn name(&self) -> Result<&'a str> {
        self.0.attribute("name").ok_or(WsError::new(
            self.0,
            WsErrorType::MalformedWsdl(WsErrorMalformedType::MissingAttribute("name".to_string())),
        ))
    }

    /// Retrieve the typename of this parameter. This refers to a type defined
    /// under the `wsdl:types` XML node.
    pub fn typename(&self) -> Result<&'a str> {
        self.0
            .attribute("element")
            .or(self.0.attribute("type"))
            .ok_or(WsError::new(
                self.0,
                WsErrorType::MalformedWsdl(WsErrorMalformedType::MissingAttribute(
                    "type".to_string(),
                )),
            ))
    }
}

/// Describes a WSDL `portType`. These describe groups of operations.
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
    pub fn operations(&self) -> Result<impl Iterator<Item = WsPortOperation>> {
        Ok(self
            .0
            .children()
            .filter(|n| n.has_tag_name("operation"))
            .map(|n| WsPortOperation(n)))
    }
}

/// Describes an operation associated with a WSDL `portType`.
/// A WSDL operation can otherwise be described as a function.
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
            .find(|n| n.has_tag_name("input"))
            .map(|n| n.attribute("message"))
            .flatten()
        {
            Some(n) => n,
            None => return Ok(None),
        };

        let (_message_namespace, message_name) =
            split_qualified(message_typename).map_err(|e| WsError::new(self.0, e))?;

        let wsdl = Wsdl::<'a, 'input>(self.0.document());
        Ok(Some(
            wsdl.messages()?
                .find(|n| n.0.attribute("name") == Some(message_name))
                .ok_or(WsError::new(
                    self.0,
                    WsErrorType::InvalidReference(message_name.to_string()),
                ))?,
        ))
    }

    /// Retrieve the output message for this port.
    pub fn output(&self) -> Result<Option<WsMessage<'a, 'input>>> {
        let message_typename = match self
            .0
            .children()
            .find(|n| n.has_tag_name("output"))
            .map(|n| n.attribute("message"))
            .flatten()
        {
            Some(n) => n,
            None => return Ok(None),
        };

        let (_message_namespace, message_name) =
            split_qualified(message_typename).map_err(|e| WsError::new(self.0, e))?;

        let wsdl = Wsdl::<'a, 'input>(self.0.document());
        Ok(Some(
            wsdl.messages()?
                .find(|n| n.0.attribute("name") == Some(message_name))
                .ok_or(WsError::new(
                    self.0,
                    WsErrorType::InvalidReference(message_name.to_string()),
                ))?,
        ))
    }

    /// Retrieve the fault message for this port.
    pub fn fault(&self) -> Result<Option<WsMessage<'a, 'input>>> {
        let message_typename = match self
            .0
            .children()
            .find(|n| n.has_tag_name("fault"))
            .map(|n| n.attribute("message"))
            .flatten()
        {
            Some(n) => n,
            None => return Ok(None),
        };

        let (_message_namespace, message_name) =
            split_qualified(message_typename).map_err(|e| WsError::new(self.0, e))?;

        let wsdl = Wsdl::<'a, 'input>(self.0.document());
        Ok(Some(
            wsdl.messages()?
                .find(|n| n.0.attribute("name") == Some(message_name))
                .ok_or(WsError::new(
                    self.0,
                    WsErrorType::InvalidReference(message_name.to_string()),
                ))?,
        ))
    }
}

/// A WSDL binding that describes how the operations in a port type
/// are bound to/from the wire.
///
/// For example, for an HTTP transport, this would contain the request
/// verb (GET/POST).
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

        let wsdl = Wsdl::<'a, 'input>(self.0.document());
        wsdl.port_types()?
            .find(|n| n.0.attribute("name") == Some(port_name))
            .ok_or(WsError::new(
                self.0,
                WsErrorType::InvalidReference(port_name.to_string()),
            ))
    }

    pub fn operations(&self) -> impl Iterator<Item = WsBindingOperation> {
        self.0
            .children()
            .filter(|n| n.has_tag_name("operation"))
            .map(|n| WsBindingOperation(n))
    }
}

/// A WSDL binding for an operation corresponding to our [WsBinding]'s associated [WsPortType].
///
/// This describes how to call this operation over the transport. For example, for SOAP,
/// this would contain the specific value of the `SOAPAction` HTTP header that is paired
/// with the request.
pub struct WsBindingOperation<'a, 'input>(Node<'a, 'input>);

impl<'a, 'input> WsBindingOperation<'a, 'input> {
    /// Retrieve the name of the operation.
    pub fn name(&self) -> Result<&'a str> {
        self.0.attribute("name").ok_or(WsError::new(
            self.0,
            WsErrorType::MalformedWsdl(WsErrorMalformedType::MissingAttribute("name".to_string())),
        ))
    }

    /// Retrieve the transport information for this operation.
    pub fn transport_operation(&self) -> Result<WsBindingTransportOperation<'a>> {
        let operation = self
            .0
            .children()
            .find(|n| n.has_tag_name("operation"))
            .ok_or(WsError::new(
                self.0,
                WsErrorType::MalformedWsdl(WsErrorMalformedType::MissingElement(
                    "operation".to_string(),
                )),
            ))?;

        match operation.tag_name().namespace().ok_or(WsError::new(
            operation,
            WsErrorType::MalformedWsdl(WsErrorMalformedType::MissingNamespace),
        ))? {
            "http://schemas.xmlsoap.org/wsdl/soap/" => {
                let action = operation.attribute("soapAction").ok_or(WsError::new(
                    operation,
                    WsErrorType::MalformedWsdl(WsErrorMalformedType::MissingAttribute(
                        "soapAction".to_string(),
                    )),
                ))?;

                Ok(WsBindingTransportOperation::Soap(action))
            }

            "http://schemas.xmlsoap.org/wsdl/soap12/" => {
                let action = operation.attribute("soapAction").ok_or(WsError::new(
                    operation,
                    WsErrorType::MalformedWsdl(WsErrorMalformedType::MissingAttribute(
                        "soapAction".to_string(),
                    )),
                ))?;

                Ok(WsBindingTransportOperation::Soap12(action))
            }

            "http://schemas.xmlsoap.org/wsdl/http/" => {
                let location = operation.attribute("location").ok_or(WsError::new(
                    operation,
                    WsErrorType::MalformedWsdl(WsErrorMalformedType::MissingAttribute(
                        "location".to_string(),
                    )),
                ))?;

                Ok(WsBindingTransportOperation::Http(location))
            }

            ns => {
                return Err(WsError::new(
                    operation,
                    WsErrorType::InvalidNamespace(ns.to_string()),
                ));
            }
        }
    }
}

/// This enum describes the specific transport fields associated with this operation.
///
/// For example, for SOAP,
pub enum WsBindingTransportOperation<'a> {
    /// This contains the value of the `SOAPAction` HTTP header, e.g. `http://ws.cdyne.com/WeatherWS/GetCityWeatherByZIP`
    Soap(&'a str),
    /// This contains the value of the `SOAPAction` HTTP header, e.g. `http://ws.cdyne.com/WeatherWS/GetCityWeatherByZIP`
    Soap12(&'a str),
    /// This contains the relative HTTP path, e.g. `/GetWeatherInformation`
    Http(&'a str),
}

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
        let binding_name = self.0.attribute("binding").ok_or(WsError::new(
            self.0,
            WsErrorType::MalformedWsdl(WsErrorMalformedType::MissingAttribute(
                "binding".to_string(),
            )),
        ))?;

        let wsdl = Wsdl::<'a, 'input>(self.0.document());
        wsdl.bindings()?
            .find(|n| n.0.attribute("name") == Some(binding_name))
            .ok_or(WsError::new(
                self.0,
                WsErrorType::InvalidReference(binding_name.to_string()),
            ))
    }
}

/// A WSDL service, usually describing an HTTP endpoint that serves
/// messages bound with a [WsBinding]
pub struct WsService<'a, 'input>(Node<'a, 'input>);

impl<'a> WsService<'a, '_> {
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
            .filter(|n| n.has_tag_name("port"))
            .map(|n| WsServicePort(n)))
    }
}

pub struct Wsdl<'a, 'input>(&'a Document<'input>);

impl<'a, 'input> Wsdl<'a, 'input> {
    pub fn new(document: &'a Document<'input>) -> Self {
        Self(document)
    }

    fn definitions(&self) -> Result<Node<'a, 'input>> {
        self.0
            .root()
            .children()
            .find(|n| n.has_tag_name("definitions"))
            .ok_or(WsError::new(
                self.0.root_element(),
                WsErrorType::MalformedWsdl(WsErrorMalformedType::MissingElement(
                    "definitions".to_string(),
                )),
            ))
    }

    pub fn port_types(&self) -> Result<impl Iterator<Item = WsPortType<'a, 'input>>> {
        let definitions = self.definitions()?;

        Ok(definitions
            .children()
            .filter(|n| n.has_tag_name("portType"))
            .map(|n| WsPortType(n))
            .into_iter())
    }

    pub fn messages(&self) -> Result<impl Iterator<Item = WsMessage<'a, 'input>>> {
        let definitions = self.definitions()?;

        Ok(definitions
            .children()
            .filter(|n| n.has_tag_name("message"))
            .map(|n| WsMessage(n))
            .into_iter())
    }

    pub fn bindings(&self) -> Result<impl Iterator<Item = WsBinding<'a, 'input>>> {
        let definitions = self.definitions()?;

        Ok(definitions
            .children()
            .filter(|n| n.has_tag_name("binding"))
            .map(|n| WsBinding(n))
            .into_iter())
    }

    pub fn services(&self) -> Result<impl Iterator<Item = WsService<'a, 'input>>> {
        let definitions = self.definitions()?;

        Ok(definitions
            .children()
            .filter(|n| n.has_tag_name("service"))
            .map(|n| WsService(n))
            .into_iter())
    }
}
