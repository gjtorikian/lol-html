use cool_thing::tokenizer::{LexUnit, NextOutputType, TagPreview, TextParsingModeSnapshot};
use cool_thing::transform_stream::TransformStream;
use cool_thing::Error;
use harness::tokenizer_test::chunked_input::ChunkedInput;
use harness::tokenizer_test::runners::BUFFER_SIZE;
use harness::tokenizer_test::test_outputs::TestTagPreview;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Default)]
pub struct ParsingResult {
    pub previews: Vec<TestTagPreview>,
    pub has_bailout: bool,
    pending_tag_preview: Option<TestTagPreview>,
}

impl ParsingResult {
    pub fn new(input: &ChunkedInput, initial_mode_snapshot: TextParsingModeSnapshot) -> Self {
        let mut result = ParsingResult {
            previews: Vec::new(),
            has_bailout: false,
            pending_tag_preview: None,
        };

        // TODO
        result.has_bailout = result.parse(input, initial_mode_snapshot).is_err();

        result
    }

    fn parse(
        &mut self,
        input: &ChunkedInput,
        initial_mode_snapshot: TextParsingModeSnapshot,
    ) -> Result<(), Error> {
        let result = Rc::new(RefCell::new(self));
        let pending_preview_confirmed = Rc::new(Cell::new(false));
        let lex_unit_handler = |_: &LexUnit| {};
        let tag_lex_unit_handler = |_: &LexUnit| NextOutputType::TagPreview;

        let tag_preview_handler = {
            let result = Rc::clone(&result);
            let pending_preview_confirmed = Rc::clone(&pending_preview_confirmed);

            move |tag_preview: &TagPreview| {
                result
                    .borrow_mut()
                    .add_tag_preview(tag_preview, pending_preview_confirmed.get());

                pending_preview_confirmed.set(false);

                NextOutputType::TagPreview
            }
        };

        let mut transform_stream = TransformStream::new(
            BUFFER_SIZE,
            lex_unit_handler,
            tag_lex_unit_handler,
            tag_preview_handler,
        );

        transform_stream
            .get_tokenizer()
            .get_eager_sm()
            .set_tag_confirmation_handler({
                let pending_preview_confirmed = Rc::clone(&pending_preview_confirmed);

                Box::new(move || pending_preview_confirmed.set(true))
            });

        input.parse(
            transform_stream,
            initial_mode_snapshot,
            NextOutputType::TagPreview,
        )?;

        if pending_preview_confirmed.get() {
            result.borrow_mut().store_pending_preview();
        }

        Ok(())
    }

    fn store_pending_preview(&mut self) {
        let pending_preview = self
            .pending_tag_preview
            .take()
            .expect("Tag should have a preview");

        self.previews.push(pending_preview);
    }

    fn add_tag_preview(&mut self, tag_preview: &TagPreview, pending_preview_confirmed: bool) {
        if pending_preview_confirmed {
            self.store_pending_preview();
        }

        // NOTE: it's not guaranteed that tag preview will produce
        // a tag at the end on input, it just gives matcher a hint
        // that there might be one (e.g. `<div` will not produce a
        // tag token). So we don't add tag preview unless we get a
        // confirmation for it.
        self.pending_tag_preview = Some(TestTagPreview::new(tag_preview));
    }
}
