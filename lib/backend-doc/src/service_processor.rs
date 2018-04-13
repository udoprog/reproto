//! Processor for service declarations.

use core::Loc;
use core::errors::*;
use core::flavored::{RpEndpoint, RpServiceBody};
use doc_builder::DocBuilder;
use escape::Escape;
use macros::FormatAttribute;
use processor::Processor;

define_processor!(ServiceProcessor, RpServiceBody, self,
    process => {
        self.write_doc(|| {
            let id = self.body.name.join("_");

            html!(self, section {id => &id, class => "section-content section-service"} => {
                self.section_title("service", &self.body.name)?;

                self.doc(&self.body.comment)?;

                for endpoint in &self.body.endpoints {
                    self.endpoint(endpoint)?;
                }

                self.nested_decls(self.body.decls.iter())?;
            });

            Ok(())
        })
    };

    current_package => &self.body.name.package;
);

impl<'p> ServiceProcessor<'p> {
    fn endpoint(&self, endpoint: &RpEndpoint) -> Result<()> {
        let id = format!(
            "{}_{}",
            self.body.name,
            endpoint.id_parts(Self::fragment_filter).join("_")
        );

        html!(self, h2 {class => "endpoint-title", id => id} => {
            self.name_until(&self.body.name)?;

            html!(self, span {class => "endpoint-id"} ~ Escape(endpoint.safe_ident()));
            html!(self, span {} ~ Escape("("));

            let mut it = endpoint.arguments.iter().peekable();

            while let Some(arg) = it.next() {
                html!(self, span {class => "endpoint-request-type"} => {
                    html!(self, span {class => "name"} ~ Escape(arg.ident.as_str()));
                    html!(self, span {class => "sep"} ~ Escape(":"));

                    if arg.channel.is_streaming() {
                        html!(self, span {class => "keyword"} ~ Escape("stream"));
                    }

                    let (req, _) = Loc::borrow_pair(&arg.channel);
                    self.write_type(req.ty())?;
                });
            }

            html!(self, span {} ~ Escape(")"));

            if let Some(response) = endpoint.response.as_ref().take().as_ref() {
                html!(self, span {class => "keyword"} ~ "&rarr;");

                html!(self, span {class => "endpoint-response-type"} => {
                    if response.is_streaming() {
                        html!(self, span {class => "endpoint-stream"} ~ Escape("stream"));
                    }

                    self.write_type(response.ty())?;
                });
            }

            if endpoint.ident() != endpoint.name() {
                html!(self, span {class => "keyword"} ~ Escape("as"));
                html!(self, span {} ~ Escape(endpoint.name()));
            }
        });

        self.doc(&endpoint.comment)?;
        Ok(())
    }
}
