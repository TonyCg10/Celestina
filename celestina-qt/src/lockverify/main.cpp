// celestina-lock-verify — the only thing in this shell that asks PAM whether a
// passphrase is right, and it is deliberately not part of the shell.
//
// ADR 0004 draws the boundary this program exists to hold: Celestina owns the
// lock surface and owns no password verification. So the verification runs
// here, in a process that holds no Wayland connection, no compositor object,
// no Qt event loop and no shell state. If PAM or a module below it crashes,
// what dies is a child that was about to answer one question — the lock
// surface above it is untouched, and the session stays locked.
//
// It is written in C rather than joining the Rust helper on purpose. The
// aggregate helper's rule is that later non-Qt services extend it instead of
// adding another runtime; that rule is about long-lived provider IO, and this
// is its opposite — a short-lived child spawned per attempt whose isolation is
// the point. Linking libpam directly also keeps the authentication path free
// of third-party crates, which for the one component that decides whether a
// locked machine opens is worth more than the convenience.
//
// The contract with the parent, deliberately as small as it can be:
//
//   argv:   --user <name> [--service <name>]
//   stdin:  the passphrase, then EOF. Never a command-line argument, which
//           every process on the machine can read out of /proc.
//   stdout: nothing, ever.
//   exit:   0 authenticated, 1 refused, 2 the question could not be asked.
//
// There is no success string to mis-parse and no verdict on a stream that
// could be confused with diagnostics: the exit status is the whole answer.

#include <security/pam_appl.h>

#include <cstdio>
#include <cstring>
#include <string>
#include <unistd.h>

namespace {

// What the parent gets back. Anything that is not `Authenticated` leaves the
// session locked; there is no fourth state and no partial success.
enum ExitCode {
    Authenticated = 0,
    Refused = 1,
    Unavailable = 2,
};

// The PAM service whose stack decides how this session authenticates. A
// packaged Celestina installs `/etc/pam.d/celestina-lock`; until it does, the
// caller names an existing service — `login` is the stack every distribution
// already has and the one other lockers fall back to.
//
// A service file that is not there is not the hole it looks like: PAM falls
// through to `other`, which denies on every stack this runs on (measured on
// the author's, 2026-08-14). A typo in the name therefore locks the person
// out rather than letting them in, which is the direction to fail in.
constexpr const char *defaultService = "login";

// Overwritten through a volatile pointer so the compiler may not decide the
// write is dead. `memset` on a buffer that is never read again is exactly the
// call optimizers remove, which is how passphrases outlive the code that
// meant to erase them.
void wipe(std::string &secret)
{
    volatile char *at = const_cast<volatile char *>(secret.data());
    for (std::string::size_type index = 0; index < secret.size(); ++index)
        at[index] = '\0';
    secret.clear();
}

// The passphrase, read from the pipe the parent holds. Bounded because an
// unbounded read from a pipe this process does not control is a memory bug
// waiting for a hostile parent; no real passphrase approaches the limit.
constexpr std::string::size_type maximumSecretBytes = 4096;

bool readSecret(std::string *secret)
{
    char buffer[512];
    ssize_t got = 0;
    while ((got = ::read(STDIN_FILENO, buffer, sizeof(buffer))) > 0) {
        if (secret->size() + static_cast<std::string::size_type>(got)
            > maximumSecretBytes) {
            wipe(*secret);
            return false;
        }
        secret->append(buffer, static_cast<std::string::size_type>(got));
    }
    // The parent sends one line; a trailing newline is the terminal's habit,
    // not part of what the person typed.
    while (!secret->empty()
           && (secret->back() == '\n' || secret->back() == '\r')) {
        secret->pop_back();
    }
    volatile char *scratch = reinterpret_cast<volatile char *>(buffer);
    for (size_t index = 0; index < sizeof(buffer); ++index)
        scratch[index] = '\0';
    return got == 0;
}

// PAM asks; this answers with the one secret it was given, and nothing else.
// Informational and error messages from the stack are discarded rather than
// printed: a module that echoes what it received would otherwise put the
// passphrase on this process's stderr.
int converse(
    int count,
    const struct pam_message **messages,
    struct pam_response **responses,
    void *context
)
{
    if (count <= 0 || !messages || !responses)
        return PAM_CONV_ERR;

    auto *secret = static_cast<const std::string *>(context);
    auto *replies = static_cast<struct pam_response *>(
        ::calloc(static_cast<size_t>(count), sizeof(struct pam_response)));
    if (!replies)
        return PAM_BUF_ERR;

    for (int index = 0; index < count; ++index) {
        const int style = messages[index]->msg_style;
        if (style != PAM_PROMPT_ECHO_OFF && style != PAM_PROMPT_ECHO_ON) {
            // PAM_TEXT_INFO and PAM_ERROR_MSG: acknowledged, never rendered.
            // This process has no one to render to.
            continue;
        }
        // strdup because PAM takes ownership of every response and frees it
        // with `free`.
        replies[index].resp = ::strdup(secret ? secret->c_str() : "");
        if (!replies[index].resp) {
            for (int done = 0; done < index; ++done)
                ::free(replies[done].resp);
            ::free(replies);
            return PAM_BUF_ERR;
        }
    }

    *responses = replies;
    return PAM_SUCCESS;
}

} // namespace

int main(int argc, char **argv)
{
    std::string user;
    std::string service = defaultService;

    for (int index = 1; index < argc; ++index) {
        const std::string argument = argv[index];
        if (argument == "--user" && index + 1 < argc) {
            user = argv[++index];
        } else if (argument == "--service" && index + 1 < argc) {
            service = argv[++index];
        } else {
            std::fprintf(stderr, "celestina-lock-verify: unknown argument\n");
            return Unavailable;
        }
    }
    if (user.empty()) {
        std::fprintf(stderr, "celestina-lock-verify: --user is required\n");
        return Unavailable;
    }

    std::string secret;
    if (!readSecret(&secret)) {
        wipe(secret);
        std::fprintf(stderr, "celestina-lock-verify: unreadable input\n");
        return Unavailable;
    }

    const struct pam_conv conversation = {converse, &secret};
    pam_handle_t *handle = nullptr;
    int status = ::pam_start(service.c_str(), user.c_str(), &conversation,
                             &handle);
    if (status != PAM_SUCCESS || !handle) {
        wipe(secret);
        // Named without the reason's text: which service is missing is useful
        // to an author, what a module said about the attempt is not ours to
        // repeat.
        std::fprintf(stderr,
                     "celestina-lock-verify: PAM service '%s' unavailable\n",
                     service.c_str());
        return Unavailable;
    }

    status = ::pam_authenticate(handle, 0);
    // Authentication alone is not permission to be here: an expired or
    // disabled account authenticates and must still be refused.
    if (status == PAM_SUCCESS)
        status = ::pam_acct_mgmt(handle, 0);

    wipe(secret);
    const int endStatus = ::pam_end(handle, status);
    handle = nullptr;

    if (status == PAM_SUCCESS && endStatus == PAM_SUCCESS)
        return Authenticated;
    if (status == PAM_ABORT || status == PAM_BUF_ERR
        || status == PAM_SYSTEM_ERR) {
        // The stack could not answer the question. That is not a refusal, and
        // the caller must not treat it as one it can retry past — but it is
        // still not an unlock.
        std::fprintf(stderr, "celestina-lock-verify: PAM could not answer\n");
        return Unavailable;
    }
    return Refused;
}
