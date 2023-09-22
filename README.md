# s3gw - Assisted Release Command

This tool aims at simplifying the [s3gw][1]'s project release process. It will
perform very opinionated actions against live git repositories, with as little
human intervention as possible.

**If you have writing privileges on the aquarist-labs organization, please, PLEASE, don't use this tool against the default repositories yet. It's not ready!**

## Testing the process

Not only because this tool is still under development, but also because one
might want to test the release process for some reason, the tool allows to setup
custom git repositories for the release process.

We recommend forking the various s3gw project's repositories, and use those
instead of the official s3gw project's repositories.

A few changes will be needed on your custom repositories if you really want to
test the entire process, including artifact builds and submodule updates
(without which the process actively breaks).

1. the submodules in your copy of the `s3gw.git` repository need to reflect your
   custom repositories' locations
2. the actions in the `.github/workflows/release.yaml` in `s3gw.git` will need to
   be adjusted so the release is not built as self-hosted
3. you may want to get rid of `.github/workflows/nightly.yaml` altogether
4. pushing release containers will fail if the registries in
   `.github/workflows/release.yaml` are not adjusted to a custom/private
   registry; in which case, the secrets in your custom repository should also
   reflect the registry's credentials
5. it's a good idea to get rid of `.github/workflows/release.yaml` in your
   custom `s3gw-charts.git` repository

## Usage

The tool relies on workspaces, with the intent of ensuring a controlled
environment for repositories when dealing with the release process. We intend,
by design, to ensure that the release is performed from different directories
than those that you may be using for development.

### Create a workspace

To start using the tool, a new workspace needs to be created (e.g., in
`/tmp/arc-workspace`):

`# arc ws init /tmp/arc-workspace`

This command will require some information to be provided, including your name,
email address, and signing key ID. A GitHub token will be asked, but it's not
currently used. If you don't want to create a Personal Access Token at this
time, feel free to just type `ghp_asdasd` or any other string begining with
`ghp_`.

It will proceed to ask you for custom repositories. Please ensure you do use
custom repositories.

Once the setup is done, repositories will be downloaded. This can take a while,
mostly because the `ceph.git` repository takes its sweet time to download.
You'll see some progress bars to keep you entertained.

Once the command finishes, all actions are to be performed in the
`/tmp/arc-workspace` directory.

### Working with releases

There are two different approaches to handling a release:

1. Starting a new release version from scratch
2. Continuing an already started, but not yet finished, release version

The first approach presumes the release process for a given version has not been
started, either by you or by someone else. The second approach presumes that
either you or someone else has started a version's release process but has yet
to finish it.

Typically you'll want to start a new release (e.g., `v0.99.0`):

`# arc rel start --notes /path/to/release-notes.md 0.99.0`

This command will ensure branches are cut for the base release version `0.99` if
they don't yet exist, and will then proceed to start a new release candidate
version for `v0.99.0`.

On the other hand, should the release have already been started, either by you
or someone else, you'll want to continue the existing release process with

`# arc rel continue --notes /path/to/release-notes.md [--version 0.99.0]`

A bit more context on this command is required. As you can see, there's an
optional `version` argument that can be provided. This argument is relevant if
your workspace has not been the one starting the release you intend to continue.

Starting a release on a workspace creates state that reflect the release being
worked on, and will remain there until the release is either finished or the
state is explicitely removed by you (i.e., `rm .arc/release.json`). However,
when you are continuing a release started by someone else, or on a different
workspace, you will want to specify which release version you intend to
continue. State will then be populated.

Continuing a release means creating a new release candidate. The tool will
ascertain what is the latest release candidate for the given version, and will
increase the release candidate number by one.

Finally, once you are done with testing, or fixing, the release, you will want
to finish the release:

`# arc rel finish [--version v0.99.0]`

The same reasoning applies to this command with regard to the `version` argument
as it does for the `rel continue` command.

## Caveats

1. At the moment, the only repositories being considered for release are
   `ceph.git`, `s3gw-ui.git`, `s3gw-charts.git`, and `s3gw.git`.

2. The release process needs polishing (or even fixing) to match what the
   project currently does.

## Building

Running `cargo build` should do the trick. By default, the resulting binary will
be located in `./target/debug/arc`. Add `--release` to build an optimized binary
instead.

## Debugging

Debug logging can be enabled by setting the `ARC_DEBUG` environment variable.
By default, the only logging that will happen is in case of error, but you may
specify a minimum log level to the application according to those supported by
`RUST_LOG` -- i.e., `trace`, `debug`, `info`, `warn`, `error`.

E.g., `# ARC_DEBUG="trace" arc help`

## LICENSE

    Copyright 2023 SUSE LLC

    Licensed under the Apache License, Version 2.0 (the "License");
    you may not use this file except in compliance with the License.
    You may obtain a copy of the License at

        http://www.apache.org/licenses/LICENSE-2.0

    Unless required by applicable law or agreed to in writing, software
    distributed under the License is distributed on an "AS IS" BASIS,
    WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
    See the License for the specific language governing permissions and
    limitations under the License.

[1]: https://github.com/aquarist-labs/s3gw
