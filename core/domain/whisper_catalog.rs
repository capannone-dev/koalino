//! Lists the Whisper models the app can download, with their pinned sizes and SHA-256 hashes.
//!
//! Sizes and hashes come from `ggerganov/whisper.cpp` on Hugging Face (2026-10-31). A file is used
//! only if it matches its hash, so changing what the server sends can't change what the app runs.
//! `small.en-tdrz` is left out: it comes from another repository and its speaker-turn output is
//! unused (#67).

/// One downloadable model file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WhisperModelEntry {
    file_name: &'static str,
    size_bytes: u64,
    sha256: &'static str,
}

impl WhisperModelEntry {
    /// Returns the file name, such as `ggml-small-q5_1.bin`.
    pub fn file_name(&self) -> &'static str {
        self.file_name
    }

    /// Returns the exact file size.
    pub fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    /// Returns the expected SHA-256, in lowercase hex.
    pub fn sha256(&self) -> &'static str {
        self.sha256
    }

    /// Returns the name shown to people, such as `small.en-q5_1`.
    pub fn name(&self) -> &'static str {
        self.file_name
            .trim_start_matches("ggml-")
            .trim_end_matches(".bin")
    }

    /// Returns the size group, such as `large-v3-turbo`, without language or quantization.
    pub fn family(&self) -> &'static str {
        let name = self.name();
        let without_quantization = name.split("-q").next().unwrap_or(name);
        without_quantization
            .strip_suffix(".en")
            .unwrap_or(without_quantization)
    }

    /// Reports whether this version understands English only.
    pub fn is_english_only(&self) -> bool {
        self.name().contains(".en")
    }

    /// Estimates peak memory while transcribing, from measured runs (2.1 GB for a 1.6 GB file).
    pub fn estimated_memory_bytes(&self) -> u64 {
        self.size_bytes / 10 * 13
    }
}

/// Builds a catalog row.
const fn entry(
    file_name: &'static str,
    size_bytes: u64,
    sha256: &'static str,
) -> WhisperModelEntry {
    WhisperModelEntry {
        file_name,
        size_bytes,
        sha256,
    }
}

/// Every offered model, smallest family first.
pub const WHISPER_MODELS: &[WhisperModelEntry] = &[
    entry(
        "ggml-tiny.bin",
        77691713,
        "be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21",
    ),
    entry(
        "ggml-tiny-q5_1.bin",
        32152673,
        "818710568da3ca15689e31a743197b520007872ff9576237bda97bd1b469c3d7",
    ),
    entry(
        "ggml-tiny-q8_0.bin",
        43537433,
        "c2085835d3f50733e2ff6e4b41ae8a2b8d8110461e18821b09a15c40c42d1cca",
    ),
    entry(
        "ggml-tiny.en.bin",
        77704715,
        "921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f",
    ),
    entry(
        "ggml-tiny.en-q5_1.bin",
        32166155,
        "c77c5766f1cef09b6b7d47f21b546cbddd4157886b3b5d6d4f709e91e66c7c2b",
    ),
    entry(
        "ggml-tiny.en-q8_0.bin",
        43550795,
        "5bc2b3860aa151a4c6e7bb095e1fcce7cf12c7b020ca08dcec0c6d018bb7dd94",
    ),
    entry(
        "ggml-base.bin",
        147951465,
        "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe",
    ),
    entry(
        "ggml-base-q5_1.bin",
        59707625,
        "422f1ae452ade6f30a004d7e5c6a43195e4433bc370bf23fac9cc591f01a8898",
    ),
    entry(
        "ggml-base-q8_0.bin",
        81768585,
        "c577b9a86e7e048a0b7eada054f4dd79a56bbfa911fbdacf900ac5b567cbb7d9",
    ),
    entry(
        "ggml-base.en.bin",
        147964211,
        "a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002",
    ),
    entry(
        "ggml-base.en-q5_1.bin",
        59721011,
        "4baf70dd0d7c4247ba2b81fafd9c01005ac77c2f9ef064e00dcf195d0e2fdd2f",
    ),
    entry(
        "ggml-base.en-q8_0.bin",
        81781811,
        "a4d4a0768075e13cfd7e19df3ae2dbc4a68d37d36a7dad45e8410c9a34f8c87e",
    ),
    entry(
        "ggml-small.bin",
        487601967,
        "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b",
    ),
    entry(
        "ggml-small-q5_1.bin",
        190085487,
        "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb",
    ),
    entry(
        "ggml-small-q8_0.bin",
        264464607,
        "49c8fb02b65e6049d5fa6c04f81f53b867b5ec9540406812c643f177317f779f",
    ),
    entry(
        "ggml-small.en.bin",
        487614201,
        "c6138d6d58ecc8322097e0f987c32f1be8bb0a18532a3f88f734d1bbf9c41e5d",
    ),
    entry(
        "ggml-small.en-q5_1.bin",
        190098681,
        "bfdff4894dcb76bbf647d56263ea2a96645423f1669176f4844a1bf8e478ad30",
    ),
    entry(
        "ggml-small.en-q8_0.bin",
        264477561,
        "67a179f608ea6114bd3fdb9060e762b588a3fb3bd00c4387971be4d177958067",
    ),
    entry(
        "ggml-medium.bin",
        1533763059,
        "6c14d5adee5f86394037b4e4e8b59f1673b6cee10e3cf0b11bbdbee79c156208",
    ),
    entry(
        "ggml-medium-q5_0.bin",
        539212467,
        "19fea4b380c3a618ec4723c3eef2eb785ffba0d0538cf43f8f235e7b3b34220f",
    ),
    entry(
        "ggml-medium-q8_0.bin",
        823369779,
        "42a1ffcbe4167d224232443396968db4d02d4e8e87e213d3ee2e03095dea6502",
    ),
    entry(
        "ggml-medium.en.bin",
        1533774781,
        "cc37e93478338ec7700281a7ac30a10128929eb8f427dda2e865faa8f6da4356",
    ),
    entry(
        "ggml-medium.en-q5_0.bin",
        539225533,
        "76733e26ad8fe1c7a5bf7531a9d41917b2adc0f20f2e4f5531688a8c6cd88eb0",
    ),
    entry(
        "ggml-medium.en-q8_0.bin",
        823382461,
        "43fa2cd084de5a04399a896a9a7a786064e221365c01700cea4666005218f11c",
    ),
    entry(
        "ggml-large-v1.bin",
        3094623691,
        "7d99f41a10525d0206bddadd86760181fa920438b6b33237e3118ff6c83bb53d",
    ),
    entry(
        "ggml-large-v2.bin",
        3094623691,
        "9a423fe4d40c82774b6af34115b8b935f34152246eb19e80e376071d3f999487",
    ),
    entry(
        "ggml-large-v2-q5_0.bin",
        1080732091,
        "3a214837221e4530dbc1fe8d734f302af393eb30bd0ed046042ebf4baf70f6f2",
    ),
    entry(
        "ggml-large-v2-q8_0.bin",
        1656129691,
        "fef54e6d898246a65c8285bfa83bd1807e27fadf54d5d4e81754c47634737e8c",
    ),
    entry(
        "ggml-large-v3.bin",
        3095033483,
        "64d182b440b98d5203c4f9bd541544d84c605196c4f7b845dfa11fb23594d1e2",
    ),
    entry(
        "ggml-large-v3-q5_0.bin",
        1081140203,
        "d75795ecff3f83b5faa89d1900604ad8c780abd5739fae406de19f23ecd98ad1",
    ),
    entry(
        "ggml-large-v3-turbo.bin",
        1624555275,
        "1fc70f774d38eb169993ac391eea357ef47c88757ef72ee5943879b7e8e2bc69",
    ),
    entry(
        "ggml-large-v3-turbo-q5_0.bin",
        574041195,
        "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2",
    ),
    entry(
        "ggml-large-v3-turbo-q8_0.bin",
        874188075,
        "317eb69c11673c9de1e1f0d459b253999804ec71ac4c23c17ecf5fbe24e259a1",
    ),
];

/// Finds a model by its file name.
///
/// Inputs: A file name; path separators never match, so this also rejects path traversal.
/// Outputs: The entry, or `None`.
/// Side effects: None.
pub fn find_whisper_model(file_name: &str) -> Option<&'static WhisperModelEntry> {
    WHISPER_MODELS
        .iter()
        .find(|model| model.file_name == file_name)
}
