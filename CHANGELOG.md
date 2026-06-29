# Changelog

## [0.7.0](https://github.com/THernandez03/b/compare/v0.6.0...v0.7.0) (2026-06-29)


### Features

* ✨ Gold-colored version manager and program names in output ([3869d5b](https://github.com/THernandez03/b/commit/3869d5bc7da95d941e7f29cd326f3c760bcde1c3))

## [0.6.0](https://github.com/THernandez03/b/compare/v0.5.1...v0.6.0) (2026-05-24)


### Features

* ✨ Colored help, -H/-v aliases, styled info/uninstall ([a899c1e](https://github.com/THernandez03/b/commit/a899c1e91ab817f17a3eb652b6b4a45366e7b3b3))
* add binary releases and install.sh ([199fab7](https://github.com/THernandez03/b/commit/199fab713536ea20f702dfdd1dc9b7fafddc11a4))
* add nightly and edge aliases to canary channel ([350d73d](https://github.com/THernandez03/b/commit/350d73d36f564f18dde52932ff0772b19a4eca6b))
* create bunx symlink on install; fix unnecessary_wraps lint ([69dbe00](https://github.com/THernandez03/b/commit/69dbe00aed02ea999e44d7d61a275cfecede7ffd))
* display from/to version during activation ([317f4ce](https://github.com/THernandez03/b/commit/317f4ceac1a225d4a6437645cf137c60ba12ceed))
* restructure CLI, add Makefile, update README ([59aa622](https://github.com/THernandez03/b/commit/59aa6226c88bedff946a336cc1c27ad28b6aa790))
* skip activation when version is already active ([af43018](https://github.com/THernandez03/b/commit/af430186f6143cef6f5b01830571443913a56a4f))


### Bug Fixes

* 🐛 Parse canary commit SHA from release body instead of name ([1707e0d](https://github.com/THernandez03/b/commit/1707e0d0b31d991a64607b04429a1f28db835e44))
* 🐛 Strip name prefix from self-update version tag ([5a2e1bf](https://github.com/THernandez03/b/commit/5a2e1bf6b4add2192280bc9d0d60c5380d48355e))
* remove stale uninstall tests, fix needless borrow in install.rs ([7dc58e0](https://github.com/THernandez03/b/commit/7dc58e0964fdb2e0d86ed8a17743b535832ba5b8))
* resolve canary to canary-{sha} for stable cache key ([f3eed7c](https://github.com/THernandez03/b/commit/f3eed7ced425b026eb2e6dd68e664a2d700983e8))
* restore main.rs (accidentally emptied in previous push) ([d966b9b](https://github.com/THernandez03/b/commit/d966b9be5d0bea1feb4dee9b880a6c3f270cf36c))
* run tests single-threaded to avoid env-var data race between modules ([c1a8e54](https://github.com/THernandez03/b/commit/c1a8e545bd36c85c4a5e1dc457666da5b6dbebbf))
* use literal em dash instead of escaped unicode in println ([48a7a66](https://github.com/THernandez03/b/commit/48a7a66b720094bf35fec9e8c6af4e671153ff0a))


### Documentation

* 📝 Document prune --force and uninstall --yes flags ([d7c2d03](https://github.com/THernandez03/b/commit/d7c2d030e331a001dd268ea03e502d8676eb4511))
* add related projects section ([58854d5](https://github.com/THernandez03/b/commit/58854d5ff615e59458dd5e78aae5a56aa47149ff))

## [0.5.1](https://github.com/THernandez03/b/compare/b-v0.5.0...b-v0.5.1) (2026-05-24)


### Bug Fixes

* 🐛 Strip name prefix from self-update version tag ([5a2e1bf](https://github.com/THernandez03/b/commit/5a2e1bf6b4add2192280bc9d0d60c5380d48355e))

## [0.5.0](https://github.com/THernandez03/b/compare/b-v0.4.0...b-v0.5.0) (2026-05-24)


### Features

* ✨ Colored help, -H/-v aliases, styled info/uninstall ([a899c1e](https://github.com/THernandez03/b/commit/a899c1e91ab817f17a3eb652b6b4a45366e7b3b3))
* add binary releases and install.sh ([199fab7](https://github.com/THernandez03/b/commit/199fab713536ea20f702dfdd1dc9b7fafddc11a4))
* add nightly and edge aliases to canary channel ([350d73d](https://github.com/THernandez03/b/commit/350d73d36f564f18dde52932ff0772b19a4eca6b))
* create bunx symlink on install; fix unnecessary_wraps lint ([69dbe00](https://github.com/THernandez03/b/commit/69dbe00aed02ea999e44d7d61a275cfecede7ffd))
* display from/to version during activation ([317f4ce](https://github.com/THernandez03/b/commit/317f4ceac1a225d4a6437645cf137c60ba12ceed))
* restructure CLI, add Makefile, update README ([59aa622](https://github.com/THernandez03/b/commit/59aa6226c88bedff946a336cc1c27ad28b6aa790))
* skip activation when version is already active ([af43018](https://github.com/THernandez03/b/commit/af430186f6143cef6f5b01830571443913a56a4f))


### Bug Fixes

* 🐛 Parse canary commit SHA from release body instead of name ([1707e0d](https://github.com/THernandez03/b/commit/1707e0d0b31d991a64607b04429a1f28db835e44))
* remove stale uninstall tests, fix needless borrow in install.rs ([7dc58e0](https://github.com/THernandez03/b/commit/7dc58e0964fdb2e0d86ed8a17743b535832ba5b8))
* resolve canary to canary-{sha} for stable cache key ([f3eed7c](https://github.com/THernandez03/b/commit/f3eed7ced425b026eb2e6dd68e664a2d700983e8))
* restore main.rs (accidentally emptied in previous push) ([d966b9b](https://github.com/THernandez03/b/commit/d966b9be5d0bea1feb4dee9b880a6c3f270cf36c))
* run tests single-threaded to avoid env-var data race between modules ([c1a8e54](https://github.com/THernandez03/b/commit/c1a8e545bd36c85c4a5e1dc457666da5b6dbebbf))
* use literal em dash instead of escaped unicode in println ([48a7a66](https://github.com/THernandez03/b/commit/48a7a66b720094bf35fec9e8c6af4e671153ff0a))


### Documentation

* 📝 Document prune --force and uninstall --yes flags ([d7c2d03](https://github.com/THernandez03/b/commit/d7c2d030e331a001dd268ea03e502d8676eb4511))
* add related projects section ([58854d5](https://github.com/THernandez03/b/commit/58854d5ff615e59458dd5e78aae5a56aa47149ff))

## [0.4.0](https://github.com/THernandez03/b/compare/v0.3.1...v0.4.0) (2026-05-24)


### Features

* ✨ Add --force to prune and --yes/-y to uninstall ([200b6d7](https://github.com/THernandez03/b/commit/200b6d7633f2d23ed375e5c18d24961078428f23))
