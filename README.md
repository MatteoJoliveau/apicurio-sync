# Apicurio Sync Tool

Apicurio Sync is a command line tool used to synchronize artifacts between an instance of [Apicurio Registry] and
a local folder.

## Synopsis

```
$ apicurio-sync --help

apicurio-sync 0.1.0

USAGE:
    apicurio-sync [FLAGS] [OPTIONS] [SUBCOMMAND]

FLAGS:
    -d, --debug      
            Whether to print debug logs or not

    -h, --help       
            Prints help information

    -V, --version    
            Prints version information


OPTIONS:
    -f, --config-file <config>      
            The configuration file to use [env: APICURIO_SYNC_CONFIG_FILE=]  [default: apicurio-sync.yaml]

        --context-file <context>    
            The context file to use [env: APICURIO_SYNC_CONTEXT_FILE=]  [default: /home/matteo/.config/apicurio-
            sync/context.json]
        --cwd <cwd>                 
            The working directory to use. Every operation will happen inside this directory. Defaults to the current
            directory. [env: APICURIO_SYNC_WORKDIR=]

SUBCOMMANDS:
    context    Work with context
    help       Prints this message or the help of the given subcommand(s)
    info       Print registry information for debugging purposes
    init       Initializes an empty config file
    sync       Synchronizes artifacts with the registry
    update     Updates the project lockfile with the registry without updating the artifacts themselves
```

## Usage

### Contexts

Apicurio Sync uses a context file (located in the default user config directory, e.g. `$HOME/.config/apicurio-sync/context.json` on Linux),
to keep track of registries and their authentication credentials. Contexts are decoupled from projects and completely local, 
so that the same project can be managed in the scope on multiple registries without changing the codebase, for example
if you have a staging and a production registry.

All operations happen in the scope of a specific context, called the "current context".
To manipulate the context configuration, use the `context` subcommand.

Context information can also be provided via environment variables, making it easier to configure in CI pipelines.

```dotenv
# The context name. Optional and only for display when running `apicurio-sync context current`.
# If not set, the registry URL is used as the context name.
APICURIO_SYNC_CONTEXT_NAME

# URL of the registry to use
APICURIO_SYNC_REGISTRY_URL
```

Beware that, when using environment variables with no context file configured, `apicurio-sync context show` will throw a
`No such file or directory` error.

#### Authentication

Apicurio Sync supports two different authentication methods (in addition to simply not using any authentication mechanism):
- Basic auth, with a simple username/password pair
- OpenID Connection, using the [authorization code flow](https://auth0.com/docs/authorization/flows/authorization-code-flow)

**WARNING**: authentication via envars is not yet supported, but planned.

To authenticate, invoke the login command for the desired method:

Using basic auth
```bash
echo "my-secret-password" | apicurio-sync context login basic --username "my-user" --password-stdin

# Password is optional
apicurio-sync context login basic --username "my-user"
```

Using OIDC
```bash
apicurio-sync context login oidc https://auth.example.com --client-id my-client-id-for-apicurio-syn

# This will open the provider login page in your default browser
```

**WARNING**: authentication credentials (e.g. password, OIDC tokens) are stored in plain text in the `context.json` file
and are visible when running `apicurio-sync context show`. PROTECT THIS FILE.

#### Examples

Create a new context or update an existing with a given URL, setting it as the currently active context

```shell
$ apicurio-sync context set production --url https://registry.example.com --current
Updated context production
```

Print the current context

```shell
$ apicurio-sync context current
production
```

Change the current context to a different context

```shell
$ apicurio-sync context set local --current
Updated context local
```

Print the entire context configuration

```shell
$ apicurio-sync context show
{
  "current_context": "local",
  "contexts": {
    "local": {
      "url": "http://localhost:8080/",
      "auth": {
        "type": "basic",
        "username": "my-user",
        "password": "my-secret-password"
      }
    "production": {
      "url": "https://registry.example.com/",
      "auth": {
        "type": "none"
      }
    }
  }
}
```


### Sync

Apicurio Sync uses a declarative approach by defining the artifacts it should manage in a YAML configuration file.
See [samples/apicurio-sync.yaml](samples/apicurio-sync.yaml) for a working example.

```yaml
push:                               # Artifacts that are uploaded from the local folder to the registry
  - group: example
    artifact: push
    name: Push example              # Optional
    description: |                  # Optional
      An example Protobuf schema 
      to demonstrate the push 
      capabilities of apicurio-sync
    path: proto/example/push.proto
    type: PROTOBUF                  # Optional, if omitted the registry will try to autodetect it from the uploaded content
    labels:                         # Optional
      - example
    properties:                     # Optional
      example.com/hello: world

pull:                               # Artifacts that are downloaded from the registry to the local folder
  - group: example
    artifact: push
    path: proto/example/pull.proto
    version: 1                      # Optional, if omitted it will use the latest available version
```

To avoid downloading newer versions by mistake, the tool uses a [lockfile](samples/apicurio-sync.lock) to keep track of the currently
synchronized artifacts. When a `pull` artifact doesn't include a `version` field, the latest version available at the time
of the first sync is used and cached in the lockfile. All subsequent `sync` commands will still use that version even if newer ones
are pushed to the registry or if the `version` config field is changed. 

To update an artifact version, use the [update](#update) command.

### Update

This command fetches new artifact versions from the registry for `pull` artifacts, either the one specified in the `version` field,
or the latest available version, then updates the lockfile with the new metadata. This command **DOES NOT** download the new artifacts
from the registry, it only updates the lockfile. Once done, you can run `sync` again to pull the new versions.

## License

This repository is publicly available under the terms of the [MIT License](LICENSE).
Apicurio is copyright of [Red Hat, Inc].

[Apicurio Registry]: https://www.apicur.io/registry
[Red Hat, Inc]: https://www.redhat.com
