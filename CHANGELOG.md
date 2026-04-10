# Changelog

All notable changes to Oxidized will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.2.0](https://github.com/oxidized-mc/server/compare/v0.1.0...v0.2.0) (2026-04-10)


### ⚠ BREAKING CHANGES

* All cross-crate facade re-exports removed. Consumers must import from source crates directly.
* **game:** WorldContext.pending_light_updates field replaced by WorldContext.lighting (Mutex<WorldLighting>).
* **protocol:** indexmap workspace dep now requires serde feature
* **server:** ServerContext now requires chunk_loader and chunk_serializer fields.
* **server:** None — all changes are additive (tests + lib target).
* **server:** PlayContext.conn field renamed to conn_handle and changed from &mut Connection to &ConnectionHandle.
* TOML config field names now use is_/has_ prefix (e.g. hardcore → is_hardcore, online_mode → is_online_mode). Environment variable overrides follow the new names (e.g. OXIDIZED_GAMEPLAY_IS_HARDCORE).
* **server:** item_name_to_id() now returns -1 for unknown items instead of a hash-based fallback. item_id_to_name() returns

### 🚀 Features

* add proptest as a dev dependency and implement various improvements and error handling in server commands ([f2e6475](https://github.com/oxidized-mc/server/commit/f2e6475b1e273de7c49ff005c3a926d55b2a4729))
* **ci:** add ARM (linux/arm64) support to Docker builds ([ac4521a](https://github.com/oxidized-mc/server/commit/ac4521a823c76a57665ca8fd1d4c4eb926598eb3))
* **ci:** add automated release pipeline with cross-platform binaries ([56b6ddc](https://github.com/oxidized-mc/server/commit/56b6ddcf8fe96a6fca437f3e818d834503b3fedf))
* **ci:** add Docker image builds to GHCR ([ce64a3c](https://github.com/oxidized-mc/server/commit/ce64a3ca320ae14d494a09f9685dfabceff99e0b))
* **game:** add event system and plugin-readiness hooks ([f4e3efd](https://github.com/oxidized-mc/server/commit/f4e3efdec2b4b406f1f98bbea2b90bf0ad641418))
* **game:** add lighting engine scaffolding (R3.9) ([f7bc903](https://github.com/oxidized-mc/server/commit/f7bc90391502b52d8766c0cce365aa003cb461fd))
* **game:** add VoxelShape face occlusion for directional light blocking ([750e11e](https://github.com/oxidized-mc/server/commit/750e11e9100d12fd7e8414d517d45d93d11d3cd4))
* **game:** add worldgen scheduler scaffolding (R3.8, ADR-016) ([bf5aeef](https://github.com/oxidized-mc/server/commit/bf5aeef662968858e43d22119ff4c933383fb7e6))
* **game:** complete R5.17 entity selector completeness ([351a7f7](https://github.com/oxidized-mc/server/commit/351a7f700b861b00ba0ee693588406245ffe3485))
* **game:** entity selectors, console commands, and all vanilla command stubs ([0b45503](https://github.com/oxidized-mc/server/commit/0b45503322cf16cd580df973db99bab9799c307c))
* **game:** extend command framework with description, pagination, and player suggestions ([e9fca0f](https://github.com/oxidized-mc/server/commit/e9fca0fd33b83bb80e2223882e789908350d80d2))
* **game:** implement /gamemode command functionality ([37f710c](https://github.com/oxidized-mc/server/commit/37f710c0702c00528a5a543abc6264df808f2dfe))
* **game:** implement basic physics engine (Phase 16) ([70b7e95](https://github.com/oxidized-mc/server/commit/70b7e95e423a85182b73864cc068c597852b30f8))
* **game:** implement Brigadier command framework (phase 18) ([d32cb23](https://github.com/oxidized-mc/server/commit/d32cb234b6620208e2f12f6260d68c4184ce5f8d))
* **game:** implement chunk sending pipeline (phase 13) ([1919879](https://github.com/oxidized-mc/server/commit/19198798160c4942967ade9de536a600ebd4003c))
* **game:** implement entity framework and tracking (phase 15) ([91d6508](https://github.com/oxidized-mc/server/commit/91d65080befd943b962d6389d5c5d1c283a04055))
* **game:** implement entity selector parsing and resolution ([@a](https://github.com/a), [@e](https://github.com/e), [@p](https://github.com/p), [@r](https://github.com/r), [@s](https://github.com/s), [@n](https://github.com/n)) ([b4d4de7](https://github.com/oxidized-mc/server/commit/b4d4de7321eecc46a0daab31162b821814487deb))
* **game:** implement flat world generation (Phase 23) ([4be465a](https://github.com/oxidized-mc/server/commit/4be465a7f756f6a08a5c5bcdadb7b41d88c46370))
* **game:** implement inventory system (phase 21) ([35ca896](https://github.com/oxidized-mc/server/commit/35ca896eb77ff0fd28442e0b29be9ca105944400))
* **game:** implement lighting engine (Phase 23a) ([85a63cc](https://github.com/oxidized-mc/server/commit/85a63ccf0acbd21bcac5e1a5cc7e995846070f86))
* **game:** implement phase 23b ECS runtime integration ([27fc451](https://github.com/oxidized-mc/server/commit/27fc451879af72adbd58538976f23ab2b3c16c01))
* **game:** implement player join and play-state login sequence (phase 12) ([8e1157d](https://github.com/oxidized-mc/server/commit/8e1157d5c7c571518c6660ca421c56a081a11a2f))
* **game:** implement player movement handling ([77461b3](https://github.com/oxidized-mc/server/commit/77461b3e62fa3f1f4c3640034f7ad2bd9e2b9d92))
* **game:** implement server level + block access (Phase 11) ([8f0f317](https://github.com/oxidized-mc/server/commit/8f0f31732d2753c5e9a80f5bdd9bbea18fe556c6))
* **game:** improve command system vanilla parity ([753807d](https://github.com/oxidized-mc/server/commit/753807d858df0554b5402fbdfe9b112502ecf115))
* **game:** improve command system with translatable messages, dynamic help, and autocomplete ([cb28ed6](https://github.com/oxidized-mc/server/commit/cb28ed6666e8b9caa145527307c64140ebf2f96d))
* **game:** persistent LightEngine per world (23a.11) ([0ca8a55](https://github.com/oxidized-mc/server/commit/0ca8a55669ea6b900abd79b37db8737d3568a09a))
* **game:** scaffold ECS entity system (R3.10, ADR-018) ([50b67f2](https://github.com/oxidized-mc/server/commit/50b67f2e67482f4e56a97f1fd3a64a12c22d479a))
* **game:** wire cross-chunk light propagation into production (23a.10) ([574c8e5](https://github.com/oxidized-mc/server/commit/574c8e54c2ace791cf22655640bf04c23da1b6fd))
* **game:** wire physics lookups to block registry + slime bounce ([f0bc921](https://github.com/oxidized-mc/server/commit/f0bc921ebade5a4b0995f6bb78cd16c78b3ebb95))
* handshake and status protocol (Phase 3) ([75288af](https://github.com/oxidized-mc/server/commit/75288af62d4bb34457a7711f1df46428a06f4ef2))
* implement phase 1 bootstrap — constants, config, CLI, logging ([c34328f](https://github.com/oxidized-mc/server/commit/c34328f5258cbc5cbc850f845085d1d8bedf8e8f))
* login authentication, encryption, and compression (Phase 4) ([b647eed](https://github.com/oxidized-mc/server/commit/b647eed6fcdcac350d08ce54e07a2b1c1edf6a22))
* **nbt:** complete NBT library — binary codec, SNBT, serde (Phase 5) ([4b84917](https://github.com/oxidized-mc/server/commit/4b84917fdac97aa2f6767271bb2f0c0a20942a91))
* **protocol:** add channel types, connection handle, and connection split (R4.1 + R4.2) ([e0a2cfb](https://github.com/oxidized-mc/server/commit/e0a2cfb58e92ef63a6dfda3b342d72aec180173c))
* **protocol:** add decode + Packet impl for ClientboundCommandsPacket ([7440a9d](https://github.com/oxidized-mc/server/commit/7440a9def1887a30da9b42ab7a04f1428ea1df80))
* **protocol:** add decode + Packet impl for ClientboundLevelChunkWithLightPacket ([be15cf2](https://github.com/oxidized-mc/server/commit/be15cf2942215e0bb4a8c6ea3976fee30e7448be))
* **protocol:** add generic send_packet/decode_packet to Connection ([019ce58](https://github.com/oxidized-mc/server/commit/019ce583cd93f210119deb6eca84ae47684f1261))
* **protocol:** add Packet trait and PacketDecodeError (R2 SP1) ([bb579af](https://github.com/oxidized-mc/server/commit/bb579afeb4a08b246e66fa9a49c6f615b3d5b840))
* **protocol:** add swing, player abilities, and client info play packets ([68e92d8](https://github.com/oxidized-mc/server/commit/68e92d8ab6112917c8095b3229c29a0109ccd9e6))
* **protocol:** add tag data for configuration state ([c195b7a](https://github.com/oxidized-mc/server/commit/c195b7ac692e21622826631e64dad71ab63b640f))
* **protocol:** implement chat system — Component, packets, rate limiting ([764109e](https://github.com/oxidized-mc/server/commit/764109e6b96e2789c3fcfc1f5ca05a132fa02c5e))
* **protocol:** implement Configuration state (Phase 6) ([5dcfd8f](https://github.com/oxidized-mc/server/commit/5dcfd8f33b8a87df1d9522f5a3c8476541991b1e))
* **protocol:** implement core data types (Phase 7) ([2a2c6af](https://github.com/oxidized-mc/server/commit/2a2c6aff535b16eba7b60ad307e709de627ad55b))
* **protocol:** implement Packet trait for 56 packet structs ([19fe8cd](https://github.com/oxidized-mc/server/commit/19fe8cd20a0fe7edd183ec868408a84faa943633))
* **protocol:** implement ServerboundClientInformationPacket (Phase 6.6) ([1e72d22](https://github.com/oxidized-mc/server/commit/1e72d22d06204c1fca4feaffe0d2d5f0e0a580cc))
* **protocol:** update to Minecraft 26.1 release ([df6ebe0](https://github.com/oxidized-mc/server/commit/df6ebe04ebe76d35f12c458f8a2feca131477c03))
* **server:** add block interaction validation for build height, spawn protection, and replacement ([d1a31e8](https://github.com/oxidized-mc/server/commit/d1a31e8a2bc7e8df1c5b086f302781d42d39a1a2))
* **server:** add console tab-completion and fix /stop shutdown ([21b0a2a](https://github.com/oxidized-mc/server/commit/21b0a2aaff2b4866796638fb68ec22c16597eeea))
* **server:** add environment variable configuration overrides (R3.4) ([c1e0ac0](https://github.com/oxidized-mc/server/commit/c1e0ac00c8b54ab6573a68cbca5cad3a620e38e7))
* **server:** complete R4 hardening — compliance tests and memory budget tests (R4.9 + R4.10) ([6fc3c1b](https://github.com/oxidized-mc/server/commit/6fc3c1b58dad0ee190de85c7d32cd760d5ae80ba))
* **server:** implement block interaction (Phase 22) ([dac4899](https://github.com/oxidized-mc/server/commit/dac4899bfb9c381ae0d4899ba6794e290e15add0))
* **server:** implement keepalive and configurable color codes ([200f281](https://github.com/oxidized-mc/server/commit/200f2813f1c628d09dfeb1f77bf0710495ac0d2a))
* **server:** implement Phase 19 — world ticking ([d33d3fe](https://github.com/oxidized-mc/server/commit/d33d3feb44e03d29a310f7b71a5aaadd20ed3a66))
* **server:** implement reader task with rate limiting (R4.4) ([dfade49](https://github.com/oxidized-mc/server/commit/dfade495763a1be2fc7b3af17b549c22721a1ff2))
* **server:** implement writer task with batch flushing (R4.3) ([69cfda6](https://github.com/oxidized-mc/server/commit/69cfda6003f33e83b9ed02a58fed2f6c349fc8f6))
* **server:** integrate PLAY-state login sequence (Phase 12) ([4d0ef5a](https://github.com/oxidized-mc/server/commit/4d0ef5a9ae24bc74015be2f459ce100c3a33b67d))
* **server:** migrate play state to reader/writer task pair (R4.5-R4.8) ([782fa5a](https://github.com/oxidized-mc/server/commit/782fa5adf92e0a3aec606fe0eaf745d5ea2b3bee))
* **server:** per-player operator permissions via ops.json (R5.15) ([12cebdf](https://github.com/oxidized-mc/server/commit/12cebdf8880a55d4a9df28dae8493d28dc996b04))
* TCP listener and raw packet framing (Phase 2) ([c567963](https://github.com/oxidized-mc/server/commit/c567963d8aa7f464de08c6fa657c02e98d042852))
* **world,protocol:** complete R5.16 safety hardening & cleanup ([dc74ae2](https://github.com/oxidized-mc/server/commit/dc74ae2d17761cc2a16b1d774f7d4b162e78967d))
* **world:** add shape occlusion check to light trigger (23a.15) ([3373ac0](https://github.com/oxidized-mc/server/commit/3373ac0906cde5cbb9b3b617f96f9fec22905091))
* **world:** Enrich BlockStateEntry with physics properties (R5.2) ([18608a4](https://github.com/oxidized-mc/server/commit/18608a4f3d982c4b52d22b2d89adfe4078271238))
* **world:** implement Anvil world loading (Phase 10) ([da5e78c](https://github.com/oxidized-mc/server/commit/da5e78cfacd5ce2254d5749e5f7195edf710a986))
* **world:** implement block and item registries (Phase 8) ([af98793](https://github.com/oxidized-mc/server/commit/af98793cf07337917fba295888ac3ddc926f1240))
* **world:** implement block tag loading from vanilla data (R5.3) ([f5a5eac](https://github.com/oxidized-mc/server/commit/f5a5eacd09c21ea0d8b8d5ffbbc60c822eba44dd))
* **world:** implement chunk data structures (Phase 9) ([5bc0bc5](https://github.com/oxidized-mc/server/commit/5bc0bc5b48e364eb9354390e1141b8fa59083e2f))
* **world:** implement ChunkSkyLightSources heightmap system (23a.13) ([413cced](https://github.com/oxidized-mc/server/commit/413cced161f8cadd0b25f90aa02d846b9aa10537))
* **world:** implement world saving (phase 20) ([4279dd7](https://github.com/oxidized-mc/server/commit/4279dd7f3313bd9cb15b1713c764973160954fde))


### 🐛 Bug Fixes

* add CFR fallback for VineFlower decompilation failures ([3f4e7e5](https://github.com/oxidized-mc/server/commit/3f4e7e52fcc682a6505c2dcd42a46792fa83f62b))
* add missing crate dependencies and bootstrap server binary ([205db42](https://github.com/oxidized-mc/server/commit/205db42beebd00e74e69914f0476c0c99e4fc76d))
* **chore:** contain setup-ref output under mc-server-ref/&lt;version&gt;/ ([f7cab85](https://github.com/oxidized-mc/server/commit/f7cab853d3ba7f08df44591c80312e33bf5a8dff))
* **ci:** add permissions to release-please caller ([ba6d5a9](https://github.com/oxidized-mc/server/commit/ba6d5a9e65f4098363b86f233a28a88226c5fffd))
* **ci:** chain publish into release-please workflow ([9929c56](https://github.com/oxidized-mc/server/commit/9929c56180d12ffa75f84b645a2d2bf7103914ad))
* **ci:** make oxidized.toml copy optional in release packaging ([410076b](https://github.com/oxidized-mc/server/commit/410076bbaaabefc5986e0f4b1d5811945ee90761))
* **ci:** remove extra-files from release-please config ([156cca6](https://github.com/oxidized-mc/server/commit/156cca65a00bad789c4a19c3a4bb957943dce935))
* **ci:** resolve all CI pipeline failures ([5e17f3e](https://github.com/oxidized-mc/server/commit/5e17f3eb56e41d4c9d23c69fb3bc45c9ad2f3f70))
* **ci:** switch release-please to simple type for workspace compat ([3aedb07](https://github.com/oxidized-mc/server/commit/3aedb07c0096d012e95dca2b586020d94706494a))
* **ci:** use explicit generic updater for Cargo.toml version ([86edc42](https://github.com/oxidized-mc/server/commit/86edc4269344eb928b67bb3d9be749fe1d571936))
* clippy ([8748b24](https://github.com/oxidized-mc/server/commit/8748b24c43b9714cdc1d15d8722cba0cf6442114))
* **deny:** use allow-org for git source allowlist ([daed6b8](https://github.com/oxidized-mc/server/commit/daed6b889e9a595d33162b2578620ef9745dbae5))
* **deps:** switch from git to version deps for crates.io publishing ([86d57be](https://github.com/oxidized-mc/server/commit/86d57be4bc07fa0b96a759a5f937a68827b5d578))
* **game,world:** replace panic! with unreachable! in test assertions ([29a1a37](https://github.com/oxidized-mc/server/commit/29a1a371e516a5348b9f90ab28cc5301b923cc5b))
* **game,world:** resolve clippy warnings in test code ([6f95035](https://github.com/oxidized-mc/server/commit/6f950355abd6f004b17ddec82c7a9eeadd3831ba))
* **game:** correct entity tracking ranges and harden entity safety ([86a64f5](https://github.com/oxidized-mc/server/commit/86a64f55b287b6d645d720f4727ef14bc45d5a61))
* **game:** correct Phase 11 against vanilla 26.1 reference ([8f6b7d0](https://github.com/oxidized-mc/server/commit/8f6b7d09181340f43c77fa3365f51f757d6de546))
* **game:** correct SNEAK_SPEED constant to match vanilla ([ffb92e4](https://github.com/oxidized-mc/server/commit/ffb92e4d7930b06118d1800d808430837ad57ba4))
* **game:** darken sky column below when opaque block placed ([ed15270](https://github.com/oxidized-mc/server/commit/ed15270d2677c0a05371c24c6bd0145cfcd22294))
* **game:** discard decrease boundary entries to prevent phantom light injection ([f991ed3](https://github.com/oxidized-mc/server/commit/f991ed35cf7933ced6efd948ee2cf440bf815295))
* **game:** fix autocomplete ranges and entity arg suggestion flags ([685ac12](https://github.com/oxidized-mc/server/commit/685ac123f3c1b32ddb4ab722b862e7390b2c8f16))
* **game:** implement destroy/keep/replace modes for /setblock command ([0d04540](https://github.com/oxidized-mc/server/commit/0d045401061798c93414bcccc1261dc332c2501d))
* **game:** improve lighting engine vanilla compliance ([dc92309](https://github.com/oxidized-mc/server/commit/dc92309fd9382b7b2a888514c3b1e76e3c82f11d))
* **game:** improve Phase 21 inventory implementation ([db4f7ce](https://github.com/oxidized-mc/server/commit/db4f7cebe842466964175157bab2de923c3d05f3))
* **game:** include MotionBlockingNoLeaves in client heightmaps ([103857f](https://github.com/oxidized-mc/server/commit/103857f79ae900988b39aa3571d28405879ed6e6))
* **game:** persist player NBT data to prevent data loss ([4aa4907](https://github.com/oxidized-mc/server/commit/4aa4907556aef56408582593ca6e41ea51793b8f))
* **game:** reject NaN/Infinity in creative mode movement validation ([2f731ed](https://github.com/oxidized-mc/server/commit/2f731ed9d10fa2c4322a590828deabeb565d1764))
* **game:** send hashed world seed and difficulty lock in login packets ([d0a5fdb](https://github.com/oxidized-mc/server/commit/d0a5fdbb58807c8364ee3f45dae84e74094fe67c))
* **game:** unify pick block logic for creative and survival modes ([65f9950](https://github.com/oxidized-mc/server/commit/65f995099aaa0722129a8189f44cb3b68ce31534))
* **game:** update stale chat::style:: imports after R3-B split ([1bcb768](https://github.com/oxidized-mc/server/commit/1bcb7689c74f0238d3677d47d9a9bed93e9cd601))
* **game:** vanilla compliance fixes for lighting engine ([6082ccd](https://github.com/oxidized-mc/server/commit/6082ccdba3015d5d8b398d4316fe60bddd35cbaf))
* **nbt:** add depth limits to SNBT parser/writer and length validation ([6ffaa6c](https://github.com/oxidized-mc/server/commit/6ffaa6cf2c7b2d46160fedc6a34cacc4b0913d54))
* **nbt:** add network NBT format for protocol packets ([7dd5169](https://github.com/oxidized-mc/server/commit/7dd5169cbf3f66d91fb5fa34f86ae456bd1f35fe))
* **nbt:** replace hardcoded float literal with `std::f32::consts::PI` in roundtrip tests ([d010f0d](https://github.com/oxidized-mc/server/commit/d010f0dffffd1b34856d025a911d7bff15a98b50))
* **nbt:** resolve clippy same_item_push in depth limit test ([82fdde2](https://github.com/oxidized-mc/server/commit/82fdde2fbd55156310d77d1fd81a0cd8487d3e92))
* **nbt:** use std::f64::consts::E instead of approximate literal ([dad11a1](https://github.com/oxidized-mc/server/commit/dad11a1cc81be2e020e58c070b640a394e6f6616))
* **protocol:** allow clippy::panic in registry tests ([13bdf3c](https://github.com/oxidized-mc/server/commit/13bdf3c0f6bd68509d3498592dab8806a1f61204))
* **protocol:** correct chat wire formats and command packet types ([a5c71ee](https://github.com/oxidized-mc/server/commit/a5c71ee78c68c3fd02658b4d3107509128d30a5a))
* **protocol:** correct ClientboundKeepAlivePacket ID from 0x16 to 0x2C ([19cd891](https://github.com/oxidized-mc/server/commit/19cd89135f462e176625eb41fc54c96f1dde88d1))
* **protocol:** correct configuration state packet IDs ([6ebfd53](https://github.com/oxidized-mc/server/commit/6ebfd53e1298e35951ba8f347857b3b630cd2133))
* **protocol:** correct SetEquipment packet ID from 0x68 to 0x66 ([79b65ee](https://github.com/oxidized-mc/server/commit/79b65ee8a9be44565a50a8c7dd665f4443d399b9))
* **protocol:** encode display name in PlayerInfoUpdate packets ([b8a8550](https://github.com/oxidized-mc/server/commit/b8a8550659b0eadf8cfbec68492ca9de7ba67a11))
* **protocol:** preserve registry entry order with IndexMap ([db8c9c7](https://github.com/oxidized-mc/server/commit/db8c9c708dfd31c25f5618ffc13c3a73282462f9))
* **protocol:** resolve clippy and formatting issues ([5408ce5](https://github.com/oxidized-mc/server/commit/5408ce562990d81acadee10e41dcadaba934be68))
* **protocol:** send tag registries with empty entry lists ([8eb081d](https://github.com/oxidized-mc/server/commit/8eb081d13c21ee12650bcf759ee285ce7a1aecd8))
* **protocol:** update packet IDs for chat and motion packets ([d6b10f7](https://github.com/oxidized-mc/server/commit/d6b10f750d5099ae43ce679cf2e3d18f3b39cb2d))
* **protocol:** use action-specific field names for ClickEvent serialization ([c5ecf49](https://github.com/oxidized-mc/server/commit/c5ecf49adef538e7f577aedc0803869a0e02e1b5))
* **protocol:** use holder encoding for dimension type in Login packet ([a155977](https://github.com/oxidized-mc/server/commit/a15597766d0f61351a07645f363afc962358facc))
* **protocol:** use snake_case field names for chat component serialization ([332cd8f](https://github.com/oxidized-mc/server/commit/332cd8fbba8c7de6c51b7475fd179e1348bdd9a9))
* **protocol:** validate compressed packet size against 2 MiB limit ([61452ff](https://github.com/oxidized-mc/server/commit/61452ff87535b1086123bfcd476877605ac6854a))
* resolve all cargo clippy warnings ([c2ecabe](https://github.com/oxidized-mc/server/commit/c2ecabe8f20f37d6df1bc01a4ec11e7d5f6c89af))
* resolve all cargo clippy warnings and errors ([d83f2c7](https://github.com/oxidized-mc/server/commit/d83f2c7e45989a8e44c677ce6ef0c7e646005238))
* resolve all CI failures (advisories, licenses, Windows tests) ([8af3e9f](https://github.com/oxidized-mc/server/commit/8af3e9fe0643a2b346384a435cc5cdb6274fb70f))
* resolve all clippy warnings across workspace ([3193b02](https://github.com/oxidized-mc/server/commit/3193b0203c39734f04ff7904e7fe7e6a284a3aa7))
* resolve broken relative links and stale references ([d6abd15](https://github.com/oxidized-mc/server/commit/d6abd154482638c380a54bdd217076a2cfe71b83))
* resolve clippy warnings across workspace ([0024842](https://github.com/oxidized-mc/server/commit/0024842e4d28f29b9be7e6ebd308203f2b486407))
* resolve dead link and align lint rules with copilot instructions ([14164dd](https://github.com/oxidized-mc/server/commit/14164ddbdb2a80e12785e495b1923bbc4deb0096))
* **server:** 11 vanilla compliance bugs across movement, protocol, login, and blocks ([bb757cc](https://github.com/oxidized-mc/server/commit/bb757ccd20d3d72585ec7e748e5d526b1997f2ea))
* **server:** accept custom payload packets during configuration ([0492cad](https://github.com/oxidized-mc/server/commit/0492caddb515bfbcaa1741db3ebd77bb83390905))
* **server:** add 10s graceful shutdown timeout ([12bcb5b](https://github.com/oxidized-mc/server/commit/12bcb5b56eaa40400bc6005de47faa5ab854bbb2))
* **server:** add config timeout, teleport resend, and time broadcast clock ([9c85b1b](https://github.com/oxidized-mc/server/commit/9c85b1bf19348ba8dc02c38d5ec42c251425aaff))
* **server:** add vanilla-compliant chat input validation and suggestion cap ([03df964](https://github.com/oxidized-mc/server/commit/03df964aff036e6db1f3cbaa5f4737a4ddbd848b))
* **server:** cap player view distance to server config maximum ([5277cd8](https://github.com/oxidized-mc/server/commit/5277cd85203a8931e1269b4fea51367bc7e2353b))
* **server:** check spawn protection on clicked block, resync both positions ([1f4641d](https://github.com/oxidized-mc/server/commit/1f4641dd7f810cc8c1886e5917b15229ed5c06dc))
* **server:** chunk persistence, block states, and double-block placement ([a88ba3b](https://github.com/oxidized-mc/server/commit/a88ba3b03a9bd7877431510153d2befa9f6ab96f))
* **server:** clean up log format — remove redundant metadata ([3a9b6a0](https://github.com/oxidized-mc/server/commit/3a9b6a087f06bdc3528a141959a076742a4d64f2))
* **server:** clear is_fall_flying when player lands ([a85e1c6](https://github.com/oxidized-mc/server/commit/a85e1c62a392b3c6915173edb8274e6f1597ef20))
* **server:** complete R2 SP4 handler migration — fix decode_packet calls and send_raw patterns ([2ee22dc](https://github.com/oxidized-mc/server/commit/2ee22dcfb25b8903782d4b3e7c9590555349c01d))
* **server:** consult gamerules for movement validation ([50ed402](https://github.com/oxidized-mc/server/commit/50ed4022ddcd671b3629b3d57a42899b55535f41))
* **server:** correct 4 movement handler bugs for vanilla compliance ([fe45b39](https://github.com/oxidized-mc/server/commit/fe45b39097acea96214cbe42e0cb095a920d8eff))
* **server:** correct eye height for sneaking and game-mode reach distance ([ed4075a](https://github.com/oxidized-mc/server/commit/ed4075a28ac9b899d2f142540b21d2acf69d7290))
* **server:** correct light serialization, tick loop, block interaction, and setblock bugs ([7525ecf](https://github.com/oxidized-mc/server/commit/7525ecfa94738be541181c6522896e8c2fc94e0b))
* **server:** correct player skin metadata index and spawn centering ([f3f7a0a](https://github.com/oxidized-mc/server/commit/f3f7a0a21c052f066b141f6ae991215b79acb82c))
* **server:** derive hat visibility from model_customisation, broadcast UPDATE_HAT ([f03723a](https://github.com/oxidized-mc/server/commit/f03723ad10f5a137e86b869405ab545eed3e5c0a))
* **server:** duplicate login prevention and chat spam disconnect ([a6a6ea4](https://github.com/oxidized-mc/server/commit/a6a6ea4905231f102b447575e55781b05ac6b6ea))
* **server:** fix 6 block interaction and world persistence bugs ([05c4393](https://github.com/oxidized-mc/server/commit/05c43931b53dec72fe1227914f17caa362d60da2))
* **server:** handle signed commands, per-player feedback, dynamic status, tab list broadcasts ([1e673ff](https://github.com/oxidized-mc/server/commit/1e673ff298c2d178567c6da03ec3609001e549bc))
* **server:** multiplayer visibility, item registry, player persistence, and login ([94b9feb](https://github.com/oxidized-mc/server/commit/94b9feb07b6430c8d0aeab9696916656b3cdb054))
* **server:** persist world seed and fix biome deserialization ([3499055](https://github.com/oxidized-mc/server/commit/3499055e6ae2b70215abca6b8d340d500d46161f))
* **server:** remove [patch] overrides that break CI ([38390be](https://github.com/oxidized-mc/server/commit/38390be4381e658e78b51236ccfd7e40c990f7c1))
* **server:** replace expect() with proper error propagation for console thread spawn ([d5ab48e](https://github.com/oxidized-mc/server/commit/d5ab48e5786c64e78745c360dc90f5556e3d7ca4))
* **server:** resolve 15 vanilla compliance bugs across all subsystems ([8883342](https://github.com/oxidized-mc/server/commit/88833422a732a83eae3f32bc6c911d25f4bc0927))
* **server:** resolve 6 gameplay bugs from playtesting ([b405638](https://github.com/oxidized-mc/server/commit/b4056385094b5b7a90705c4a63773b5f1bcf8a1f))
* **server:** resolve 7 gameplay bugs ([21c01e6](https://github.com/oxidized-mc/server/commit/21c01e6329378842cf329187432980e5090447e9))
* **server:** send LEVEL_CHUNKS_LOAD_START after initial chunk batch ([f9e3306](https://github.com/oxidized-mc/server/commit/f9e33069a34b806c7874f35e098259fb8c3c828e))
* **server:** send player's own entity data for skin parts on join ([35bc430](https://github.com/oxidized-mc/server/commit/35bc430603fa64e9267a2581ed2b9ad915f498fc))
* **server:** update Dockerfile for git deps ([f38befc](https://github.com/oxidized-mc/server/commit/f38befcc17040a45d750154e5ee91f81012caa44))
* **server:** vanilla compliance — 10 behavior bugs fixed ([2cb905e](https://github.com/oxidized-mc/server/commit/2cb905e4565a46c47ee4a9cdafa9e4a3f4dec611))
* **server:** vanilla compliance — 8 behavioral bugs fixed ([acf5110](https://github.com/oxidized-mc/server/commit/acf51106fab879732aa8e5449f662fcfdd65de67))
* **server:** vanilla compliance audit — 12 behavior bugs fixed ([a5abfc1](https://github.com/oxidized-mc/server/commit/a5abfc177a1d96733b0c900185671253652fb256))
* **server:** vanilla compliance audit — 7 protocol and behavior fixes ([e9bcafc](https://github.com/oxidized-mc/server/commit/e9bcafcf4ff8fe911aa9684edef9fb129f6549ae))
* **server:** vanilla compliance fixes for 10 gameplay bugs ([3d8aff5](https://github.com/oxidized-mc/server/commit/3d8aff5ad05aca5d3ea62095ee5565c20db1d0a1))
* update repository URLs to reflect new project location ([5a9e16a](https://github.com/oxidized-mc/server/commit/5a9e16a9cfb4c2ba4fa44f95eb5d1c4b2f105185))
* **world:** allow unwrap_used in DataLayer proptests ([489a625](https://github.com/oxidized-mc/server/commit/489a62535e378c9a844b529c5a729d1039379ad3))
* **world:** complete R5.2 — add map_color, light_opacity extraction and property tests ([ce34ae3](https://github.com/oxidized-mc/server/commit/ce34ae3e3fb32876190c16814e1d476f4bde3db7))
* **world:** correct global palette bit width and improve chunk data structures ([f97b34e](https://github.com/oxidized-mc/server/commit/f97b34e481e33c947c682aa8364cfbc4d3452489))
* **world:** remove VarInt length prefix from PalettedContainer wire format ([fe0f721](https://github.com/oxidized-mc/server/commit/fe0f72170df61f04edba723ae39f812a2556f02b))
* **world:** resolve clippy warnings for CI ([b0c3522](https://github.com/oxidized-mc/server/commit/b0c35220506a1b011b8862003c10c727239c0055))
* **world:** use checked integer conversions in registry loading ([9c65421](https://github.com/oxidized-mc/server/commit/9c654219e2127012305820e9fc044dbc1b412b1b))
* **world:** use vanilla registration order for item and block registries ([cee405b](https://github.com/oxidized-mc/server/commit/cee405bbaa80d5f3b94cf1a3a84069241260462f))
* **world:** validate region file header entries and payload bounds ([5f27f3a](https://github.com/oxidized-mc/server/commit/5f27f3afee09f9d3e27300ae5db39e4ade2bfa89))


### ⚡ Performance

* **ci:** use native ARM runner instead of QEMU for Docker builds ([afbffc7](https://github.com/oxidized-mc/server/commit/afbffc7ec6c790a529e4046fa948c411d186de6f))
* **game:** add direction bitmask to BFS light propagation (23a.14) ([90f0822](https://github.com/oxidized-mc/server/commit/90f08226813b5fb2541a6b1dba16206beda41cc7))
* **nbt:** avoid cloning NbtCompound in from_compound() ([c88e19e](https://github.com/oxidized-mc/server/commit/c88e19eefab678f96bb8eaf90ed19c502117566c))
* **world:** lazy DataLayer allocation + bulk sky light fill (23a.16) ([cb36d39](https://github.com/oxidized-mc/server/commit/cb36d397d940e9a7bcdb08a8756a5e55927af50d))


### 🔨 Refactor

* add architectural review gate and #[non_exhaustive] to error enums ([2c61f32](https://github.com/oxidized-mc/server/commit/2c61f3234c8e6c1ecf7fd235c4fb63ae091a047a))
* align architecture docs with ADR decisions ([4a9894e](https://github.com/oxidized-mc/server/commit/4a9894ef39246eca5acd4dadd6440bc957c5ad94))
* apply is_/has_/can_ prefix to all public boolean fields (R3.6) ([cdf5f3b](https://github.com/oxidized-mc/server/commit/cdf5f3b27c3ae69a5ddf40386f1ab6102740636c))
* **docs:** add R5.18 phase for config cleanup and server-tunable constants ([b1801b0](https://github.com/oxidized-mc/server/commit/b1801b0bddee78025252a01e560f8751ebcc775d))
* **docs:** update phase document for R5.14–R5.17 planning and verification ([475e61e](https://github.com/oxidized-mc/server/commit/475e61e699df3177bb0e7870e2e8918cc0587eba))
* eliminate duplicate code with macros and generics ([d0df1ba](https://github.com/oxidized-mc/server/commit/d0df1ba70eaa0650bd39fff9d565390eced2adff))
* enforce strict lint levels and fix ADR-002/004 violations (R3.1) ([86a0611](https://github.com/oxidized-mc/server/commit/86a0611d64a064aa5fadfff9a8641937ce106cae))
* extract oxidized-types and oxidized-nbt to standalone repos ([34fef6c](https://github.com/oxidized-mc/server/commit/34fef6c9958d1b58abbe41b3914c84ebf98bafa5))
* **game,protocol:** fix fmt, clippy, and decode correctness ([eed9238](https://github.com/oxidized-mc/server/commit/eed923880bbcbc6ca1b2a100c8db5cdfce87dece))
* **game,server:** rewrite Phase 19 world ticking for vanilla 26.1 parity ([2002b14](https://github.com/oxidized-mc/server/commit/2002b1437a95381f9655daf0d3610c3a7cd75252))
* **game:** consolidate AABB into oxidized-protocol ([fdeb62e](https://github.com/oxidized-mc/server/commit/fdeb62e73b8efd384fc13631a0ea6d54dc3f3e53))
* **game:** declarative op permission on commands, console bypasses ([9e8cfcb](https://github.com/oxidized-mc/server/commit/9e8cfcb05a1b02353d1e1e316bbe346e62746478))
* **game:** deduplicate command types and split context module ([2a4c35b](https://github.com/oxidized-mc/server/commit/2a4c35bcad99cb2df80cdfaff25c90136f357b8d))
* **game:** extract argument dispatch helpers in command context ([af196c8](https://github.com/oxidized-mc/server/commit/af196c81326e478149491674200a87169de4a8c0))
* **game:** extract command registration helpers (R5.9) ([d876400](https://github.com/oxidized-mc/server/commit/d8764004bba847f24820f86ff0bd2c8191d7b6ab))
* **game:** extract lighting, physics, worldgen to standalone crates ([2d3766a](https://github.com/oxidized-mc/server/commit/2d3766a56df05301d8efa44b3899873c719e22c7))
* **game:** improve Phase 23 flat world generation ([917e939](https://github.com/oxidized-mc/server/commit/917e93991869eb9d13db1b7cc5faf37e158fb5ee))
* **game:** move command implementations to commands/impls/ ([86291c6](https://github.com/oxidized-mc/server/commit/86291c64f3481df464b1a572988e33078fd81ae3))
* **game:** replace embedded command engine with oxidized-commands dependency ([214728a](https://github.com/oxidized-mc/server/commit/214728a12a38a558609af002f2fda78e50c499a0))
* **game:** replace embedded inventory types with oxidized-inventory dependency ([808d586](https://github.com/oxidized-mc/server/commit/808d586426b896374525b7562944df27aea1314e))
* **game:** replace hardcoded physics properties with registry data (R5.5) ([5d51801](https://github.com/oxidized-mc/server/commit/5d5180196506a4fa6502e038451b1661fddfd69e))
* **game:** split context.rs into string_reader, argument_parser, argument_access ([2f854b3](https://github.com/oxidized-mc/server/commit/2f854b3c11064ae3ca1fdfd72b86fb35294b152c))
* **game:** use lighting engine for flat world sky light ([ce751e7](https://github.com/oxidized-mc/server/commit/ce751e7ba02da0a0051d4e202c9a9fd1c52fcf1e))
* implement architectural refactoring for ADR compliance and code quality ([fb14043](https://github.com/oxidized-mc/server/commit/fb140439741748d7173a3a137405d90b10c347f5))
* **nbt,world,game:** R6 small wins — eliminate localized duplication ([bc19739](https://github.com/oxidized-mc/server/commit/bc1973903fd99f44abe91c910cf177efd1e41b86))
* **nbt:** complete Phase R3 — split serde.rs, rename should_authenticate ([040391f](https://github.com/oxidized-mc/server/commit/040391f3a85611af29dc7118e8f0b22bdfa3e6fb))
* phase 1 lifecycle retrofit — structured logging, unknown key preservation, 22 new tests ([232be1f](https://github.com/oxidized-mc/server/commit/232be1f4686f68968fb553576c8ebdc2a60579c6))
* **phases:** update R5 milestones and unify data extraction architecture ([18cc885](https://github.com/oxidized-mc/server/commit/18cc885058b5b292eeba47ef683aea4f65c6a359))
* **prompt:** rename vanilla-compliance-fix to vanilla-compliance-audit ([da6f070](https://github.com/oxidized-mc/server/commit/da6f070f429baa4dc0395090b75a4de29308e7b2))
* **protocol:** eliminate type/codec boilerplate with declarative macros ([b189308](https://github.com/oxidized-mc/server/commit/b1893082cd29872bf1c6f7458d10e95ee9f83ee4))
* **protocol:** extract packet codec helpers & roundtrip test macro (R5.8) ([d9770da](https://github.com/oxidized-mc/server/commit/d9770da0d5e338b6d93dbfdcf9de495daebbd342))
* **protocol:** extract TextColor, ClickEvent, HoverEvent from style.rs ([bf55a64](https://github.com/oxidized-mc/server/commit/bf55a64cd8533666e65c1682b8f665efe4ebaa57))
* **protocol:** extract transport module to oxidized-transport crate ([a94d4ff](https://github.com/oxidized-mc/server/commit/a94d4ff7d90eaebe157df444ca195e7280b5e847))
* **protocol:** fix clippy warnings and fmt in chat module ([06e3f2d](https://github.com/oxidized-mc/server/commit/06e3f2d5ab0b9734a1174e45570a7004692ad3ea))
* **protocol:** group connection, compression, crypto into transport/ ([49eef4f](https://github.com/oxidized-mc/server/commit/49eef4ff0bfc90519e7230414f72d1827a49c8d0))
* **protocol:** remove per-packet error types and inherent methods (R2-SP5) ([0a24748](https://github.com/oxidized-mc/server/commit/0a247489fb997131859c34b2b88ed5d0e6948315))
* **protocol:** replace extracted modules with re-exports ([5993dc6](https://github.com/oxidized-mc/server/commit/5993dc6d5629fb7478a1d78e47a0200c8f4c97ed))
* **protocol:** split component.rs into JSON and NBT serialization modules ([4fa1487](https://github.com/oxidized-mc/server/commit/4fa1487c01222724c895fc6c2e6d3ce458f56416))
* **protocol:** split serverbound_move_player into 4 Packet impls ([4ff1536](https://github.com/oxidized-mc/server/commit/4ff153696187188ab2779e257fe4ae670ed5bcb8))
* **protocol:** standardize packet decoder error handling (R5.10) ([1e4f306](https://github.com/oxidized-mc/server/commit/1e4f306cdc46bf911cc70b9a58b80c2e0a80754b))
* remove facade re-exports, switch to crates.io deps ([a8b6b17](https://github.com/oxidized-mc/server/commit/a8b6b1781c73f2c3a1bf6a40b9cc30b611ad3ed6))
* remove phase references from test section comments ([00481fc](https://github.com/oxidized-mc/server/commit/00481fc023444e74ab0cffe2563e3321564196a8))
* remove unnecessary re-export modules ([1561e2a](https://github.com/oxidized-mc/server/commit/1561e2a284932503a943b17b99aa52faef48e3cb))
* replace magic numbers with named constants (R5.13) ([ac3baf0](https://github.com/oxidized-mc/server/commit/ac3baf05746e668af3c29ca5412cc84ef18ebd5b))
* replace server.properties with TOML configuration ([d962f09](https://github.com/oxidized-mc/server/commit/d962f09bf11f111aff9b1177ee3cd48d06d981cd))
* **server,world,game:** break down long functions & reduce nesting ([d559c23](https://github.com/oxidized-mc/server/commit/d559c2319540ee246306e6725cc753bfbab96bfb))
* **server:** config cleanup — remove Java leftovers & extract hardcoded values ([b490ee5](https://github.com/oxidized-mc/server/commit/b490ee5830f0fa4fd3094967591ebc68120f8b24))
* **server:** decompose oversized structs (R5.11) ([9a677f2](https://github.com/oxidized-mc/server/commit/9a677f29a94558d71016f22d092b36e419ec2267))
* **server:** enrich logs across all severity levels ([e859800](https://github.com/oxidized-mc/server/commit/e859800a5163e1993c9998e7df6c8e36fd8466a4))
* **server:** group cli, logging, console into app/ module ([c038113](https://github.com/oxidized-mc/server/commit/c03811378ac1267bada2767c0436ec8518acb549))
* **server:** harden Phase 22 block interaction ([9053864](https://github.com/oxidized-mc/server/commit/9053864bc592f697326be1517f8a5ddcabd9a6d7))
* **server:** move tick loop to dedicated OS thread (ADR-019) ([0d0f7c1](https://github.com/oxidized-mc/server/commit/0d0f7c12e89f62864a8add7d0e4385eb91fa49a0))
* **server:** replace manual encode/decode with send_packet and decode_packet ([00db68b](https://github.com/oxidized-mc/server/commit/00db68b6fd7e21c335855ffceb8a243ef82065cc))
* **server:** replace NoopServerHandle with Weak&lt;Self&gt; self-ref ([251462f](https://github.com/oxidized-mc/server/commit/251462feed7aea036469355fd523fa7a75420f35))
* **server:** replace string-based block categorization with flags/tags (R5.4) ([e4a69db](https://github.com/oxidized-mc/server/commit/e4a69dbecbf49ab7b2410f268f61366254a56f84))
* **server:** split config.rs into config/ module directory ([3a8dc9a](https://github.com/oxidized-mc/server/commit/3a8dc9abb5377123d9b4526ac9e759a8a685de47))
* **server:** split network.rs into network/ module tree ([3854ea1](https://github.com/oxidized-mc/server/commit/3854ea1201705198f6babd457237c55d9cfa21e2))
* **server:** split oversized play/mod.rs and block_interaction.rs (R3.5) ([845bc21](https://github.com/oxidized-mc/server/commit/845bc217cf1ea0c796ac5021901f6f2934dacef7))
* **types:** deduplicate ChunkPos into oxidized-types crate ([dad5201](https://github.com/oxidized-mc/server/commit/dad5201dc96dc77b7a7f190fa18ac1f3aa653590))
* **world,server:** improve Phase 20 world saving implementation ([a8463cd](https://github.com/oxidized-mc/server/commit/a8463cd7eb1843937b661d021104ba15b28f47b3))
* **world:** compile-time item ID codegen (R5.7) ([e215b14](https://github.com/oxidized-mc/server/commit/e215b142009ffd13bc1b953ff896b87dba5b263d))
* **world:** enrich BlockStateFlags with data-driven block properties (R5.1) ([e5c9ee2](https://github.com/oxidized-mc/server/commit/e5c9ee20954112fc821d252d7f65bbee58616fd0))
* **world:** extract palette wire-format helpers to palette_codec module ([0507a9d](https://github.com/oxidized-mc/server/commit/0507a9d6981cda630e561b1ec6dd100e44b489f3))
* **world:** replace embedded registry with oxidized-registry dependency ([9c7a1de](https://github.com/oxidized-mc/server/commit/9c7a1de5efe4e2c82c9bad84868f14eb394aef70))
* **world:** replace hardcoded biome resolution with registry lookup (R5.6) ([37bd2d0](https://github.com/oxidized-mc/server/commit/37bd2d0f197d56966c9b4ba39b24e589dfff9114))
* **world:** replace runtime block registry with build.rs codegen ([becabb7](https://github.com/oxidized-mc/server/commit/becabb7159b84c274cc66f224db6afc51779226a))
* **world:** use extracted oxidized-chunks and oxidized-anvil crates ([d093ded](https://github.com/oxidized-mc/server/commit/d093dedada15cc6a4002549336833ecbd3b39cb7))

## [Unreleased]

### Added
- Cargo workspace with six crates: `oxidized-server`, `oxidized-protocol`,
  `oxidized-nbt`, `oxidized-world`, `oxidized-game`, `oxidized-macros`
- Repository scaffolding: README, licenses (MIT/Apache-2.0), CONTRIBUTING,
  CODE_OF_CONDUCT, SECURITY, CI workflow
- Rust tooling: `rustfmt.toml`, `deny.toml`, `rust-toolchain.toml`
- Decompiled Minecraft 26.1-pre-3 reference (`mc-server-ref/decompiled/`, 4 789 files)
- 38 detailed implementation phase documents
- 34 Architecture Decision Records
- Architecture documentation: system overview, crate layout, protocol, world format,
  entity system — aligned with all ADRs
- Reference documentation: Java class map (110+ mappings), binary format specs,
  protocol packet listing
- Server binary bootstrap: mimalloc global allocator, Tokio runtime, structured logging
  (Phase 1)
- TOML-based server configuration with full type safety and serde derives (Phase 1)
- GitHub issue templates (bug report, feature request, question)
- Pull request template
- Dependabot configuration for Cargo and GitHub Actions
- Development lifecycle with 9 stages, 5 feedback loops, and quality gates
- TCP listener with raw packet framing and VarInt/VarLong codec (Phase 2)
- Connection struct with protocol state tracking (Phase 2)
- Handshake + Status protocol — server appears in Minecraft multiplayer list with
  correct MOTD, version, and player count (Phase 3)
- Protocol dispatch for Handshaking and Status connection states (Phase 3)
- Wire type helpers: String, u16, i64 read/write for packet codec (Phase 3)
- Login authentication with Mojang session server (online mode) and offline UUID
  derivation (Phase 4)
- RSA-1024 key exchange and AES-128-CFB8 stream encryption (Phase 4)
- Zlib compression with configurable threshold (Phase 4)
- Login packet structs: Hello, Key, Compression, LoginFinished, Disconnect (Phase 4)
- Full encrypted + compressed connection pipeline with transparent I/O (Phase 4)
- Complete NBT library: all 13 tag types, binary codec, Modified UTF-8, NbtAccounter,
  GZIP/zlib I/O, SNBT parser+formatter, serde integration (Phase 5)

### Changed
- Configuration format from Java `.properties` to TOML

### Security
- URL-encode all query parameters in Mojang session authentication (Phase 4)
- Replaced deprecated `rustsec/audit-check@v2` CI action with direct `cargo-audit`

[Unreleased]: https://github.com/oxidized-mc/server/commits/main
