//! Decides which screen frames are new slides (Issue #49).
//!
//! The method follows lecture-video slide extraction: block-based frame differencing on a
//! downscaled grayscale frame. Each frame's luma is area-averaged into a 32x18 grid. A cell has
//! changed when its average moves by at least `CELL_CHANGE_THRESHOLD` levels, and two frames
//! differ when at least `MIN_CHANGED_CELLS` cells (about 1 %) changed: a mouse cursor or
//! compression noise touches one or two cells, while a new line of text touches many. Per-pixel
//! differences are too sensitive and 64-bit perceptual hashes too coarse for slides.
//!
//! A frame is saved once it is stable (it matches the previous sample, so transitions,
//! animations and videos are skipped) and differs from the last saved slide (so a slide shown
//! for minutes is saved once). Only the small grids are kept, never the frames. The constants are
//! starting points to tune on real shares.

use crate::ports::screen_share::ScreenFrame;

/// Columns of the comparison grid (16:9, like most shared screens).
const GRID_COLUMNS: usize = 32;
/// Rows of the comparison grid.
const GRID_ROWS: usize = 18;
/// Smallest change of a cell's average luma, in levels of 255, that counts as changed.
const CELL_CHANGE_THRESHOLD: u8 = 8;
/// Fewest changed cells that make two frames differ (6 of 576, about 1 %).
const MIN_CHANGED_CELLS: usize = 6;

/// A frame reduced to the average luma of each grid cell, row by row.
#[derive(Clone, Debug, Eq, PartialEq)]
struct FrameGrid([u8; GRID_COLUMNS * GRID_ROWS]);

impl FrameGrid {
    /// Area-averages a frame into the grid.
    ///
    /// Inputs: A valid frame of any size. Outputs: The grid; a cell no pixel falls into (frames
    /// smaller than the grid) stays 0 in every frame of that size. Side effects: None.
    fn from_frame(frame: &ScreenFrame) -> Self {
        let mut cell_sums = [0_u64; GRID_COLUMNS * GRID_ROWS];
        let mut cell_pixel_counts = [0_u64; GRID_COLUMNS * GRID_ROWS];
        for (row_index, row) in frame.luma().chunks_exact(frame.width()).enumerate() {
            let grid_row = row_index * GRID_ROWS / frame.height();
            for (column_index, &luma) in row.iter().enumerate() {
                let cell_index =
                    grid_row * GRID_COLUMNS + column_index * GRID_COLUMNS / frame.width();
                cell_sums[cell_index] += u64::from(luma);
                cell_pixel_counts[cell_index] += 1;
            }
        }
        let mut cells = [0_u8; GRID_COLUMNS * GRID_ROWS];
        for ((cell, sum), pixel_count) in cells.iter_mut().zip(cell_sums).zip(cell_pixel_counts) {
            if let Some(average) = sum.checked_div(pixel_count) {
                // The average of 8-bit values always fits in 8 bits.
                *cell = u8::try_from(average).unwrap_or(u8::MAX);
            }
        }
        Self(cells)
    }

    /// Tells whether two grids show different content.
    ///
    /// Inputs: Another grid. Outputs: `true` when at least `MIN_CHANGED_CELLS` cells moved by
    /// `CELL_CHANGE_THRESHOLD` or more. Side effects: None.
    fn differs_from(&self, other: &Self) -> bool {
        let changed_cell_count = self
            .0
            .iter()
            .zip(other.0.iter())
            .filter(|(cell, other_cell)| cell.abs_diff(**other_cell) >= CELL_CHANGE_THRESHOLD)
            .count();
        changed_cell_count >= MIN_CHANGED_CELLS
    }
}

/// Remembers the previous sample and the last saved slide of one screen share.
#[derive(Debug, Default)]
pub struct SlideChangeDetector {
    previous_sample: Option<FrameGrid>,
    last_saved_slide: Option<FrameGrid>,
}

impl SlideChangeDetector {
    /// Creates a detector that has seen nothing yet.
    ///
    /// Inputs: None. Outputs: The detector. Side effects: None.
    pub fn new() -> Self {
        Self::default()
    }

    /// Tells whether `frame` is a new slide to save.
    ///
    /// Inputs: The latest sample of the shared screen, taken at a regular interval.
    /// Outputs: `true` when the frame matches the previous sample and differs from the last
    /// saved slide (or none was saved yet); the frame then becomes the last saved slide.
    /// Side effects: Keeps the frame's grid (not the frame) for the next call.
    pub fn observe(&mut self, frame: &ScreenFrame) -> bool {
        let grid = FrameGrid::from_frame(frame);
        let is_stable = self
            .previous_sample
            .as_ref()
            .is_some_and(|previous_sample| !previous_sample.differs_from(&grid));
        let is_new_slide = is_stable
            && self
                .last_saved_slide
                .as_ref()
                .is_none_or(|last_saved_slide| last_saved_slide.differs_from(&grid));
        if is_new_slide {
            self.last_saved_slide = Some(grid.clone());
        }
        self.previous_sample = Some(grid);
        is_new_slide
    }

    /// Forgets everything seen, as when a share ends.
    ///
    /// Inputs: None. Outputs: None. Side effects: Clears both remembered grids.
    pub fn reset(&mut self) {
        self.previous_sample = None;
        self.last_saved_slide = None;
    }
}
