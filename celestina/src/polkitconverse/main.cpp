// celestina-polkit-converse — the only thing in this shell that talks to
// polkit's authentication helper, and it is deliberately not part of the shell.
//
// ADR 0005 draws the same boundary ADR 0004 drew for the lock: Celestina owns
// the prompt and owns no verification. So the conversation runs here, in a
// process that holds no Wayland connection, no compositor object and no Qt
// event loop, and even here nothing decides anything — `PolkitAgentSession`
// hands the response to `polkit-agent-helper-1`, which runs PAM as root and
// tells polkitd itself whether the person authenticated. This program never
// learns why an attempt succeeded and has no branch that could invent one.
//
// It links libpolkit-agent-1 rather than spawning
// `/usr/lib/polkit-1/polkit-agent-helper-1` directly, and that is not a
// convenience. On this machine the helper is not setuid: it is reached over
// `/run/polkit/agent-helper.socket`, socket-activated by systemd, and it
// refuses to run any other way ("needs to be setuid root"). Which of the two
// transports a machine offers is polkit's business and changes between
// releases; the library is where polkit keeps that knowledge, so spawning the
// binary ourselves would be a hand-written copy of a decision we do not own.
// The delegation ADR 0005 requires is unchanged — the helper still verifies,
// and this process still cannot.
//
// The contract with the parent, deliberately as small as it can be:
//
//   argv:   --user <name>
//   stdin:  the cookie on the first line; afterwards one percent-encoded
//           response line each time this program asks for one.
//   stdout: one event per line, percent-encoded so no prompt can forge a
//           frame:
//             secret <prompt>    ask, and do not echo what is typed
//             visible <prompt>   ask, and echo it
//             info <text>        say this
//             problem <text>     say this went wrong
//   exit:   0 authenticated, 1 refused, 2 the question could not be asked.
//
// The verdict is the exit status and never a word on stdout, for the reason
// the lock's verifier does the same: there is no success string to mis-parse.
// The response travels one way, from the parent's pipe into the session, and
// is written to no stream this program owns.

// polkit's agent headers refuse to compile until a caller says out loud that
// this API changes between releases. Acknowledged here rather than hidden in a
// build flag: it is the reason this program is small and isolated, and the
// reason a polkit upgrade is something to rebuild against rather than to
// discover through the shell misbehaving.
#define POLKIT_AGENT_I_KNOW_API_IS_SUBJECT_TO_CHANGE
#define POLKIT_I_KNOW_API_IS_SUBJECT_TO_CHANGE

#include <polkit/polkit.h>
#include <polkitagent/polkitagent.h>

#include <glib-unix.h>

#include <cstdio>
#include <cstring>
#include <string>

namespace {

// The parent's whole vocabulary for an outcome, shared with the lock's
// verifier so both boundaries answer in the same three words.
enum ExitCode {
    Authenticated = 0,
    Refused = 1,
    Unavailable = 2,
};

struct Conversation {
    PolkitAgentSession *session = nullptr;
    GMainLoop *loop = nullptr;
    int outcome = Unavailable;
    // Set the moment the session says it completed, so a helper that dies
    // afterwards cannot turn a decided attempt into an undecided one.
    bool decided = false;
};

// Percent-encoding, hand-rolled because this process links no Qt and the rule
// it has to hold is one sentence: nothing a prompt or a passphrase contains
// may look like a frame boundary. Everything outside the unreserved set goes
// out as %XX, so a newline inside a PAM message cannot invent a second event.
std::string encode(const char *text)
{
    static const char *digits = "0123456789ABCDEF";
    std::string out;
    if (!text)
        return out;
    for (const unsigned char *at = reinterpret_cast<const unsigned char *>(text);
         *at != '\0'; ++at) {
        const unsigned char byte = *at;
        const bool unreserved = (byte >= 'A' && byte <= 'Z')
            || (byte >= 'a' && byte <= 'z') || (byte >= '0' && byte <= '9')
            || byte == '-' || byte == '.' || byte == '_' || byte == '~';
        if (unreserved) {
            out.push_back(static_cast<char>(byte));
        } else {
            out.push_back('%');
            out.push_back(digits[byte >> 4]);
            out.push_back(digits[byte & 0x0F]);
        }
    }
    return out;
}

int hexValue(char digit)
{
    if (digit >= '0' && digit <= '9')
        return digit - '0';
    if (digit >= 'A' && digit <= 'F')
        return digit - 'A' + 10;
    if (digit >= 'a' && digit <= 'f')
        return digit - 'a' + 10;
    return -1;
}

// Refuses malformed input rather than repairing it. A response this program
// cannot decode exactly is one it must not hand to PAM half-read.
bool decode(const std::string &text, std::string *out)
{
    out->clear();
    for (std::string::size_type index = 0; index < text.size(); ++index) {
        if (text[index] != '%') {
            out->push_back(text[index]);
            continue;
        }
        if (index + 2 >= text.size())
            return false;
        const int high = hexValue(text[index + 1]);
        const int low = hexValue(text[index + 2]);
        if (high < 0 || low < 0)
            return false;
        out->push_back(static_cast<char>((high << 4) | low));
        index += 2;
    }
    return true;
}

// Overwritten through a volatile pointer, for the reason the lock's verifier
// does the same: a clear of a buffer nothing reads again is what a compiler
// removes, and that is how passphrases outlive the code that meant to erase
// them.
void wipe(std::string &secret)
{
    volatile char *at = const_cast<volatile char *>(secret.data());
    for (std::string::size_type index = 0; index < secret.size(); ++index)
        at[index] = '\0';
    secret.clear();
}

void emitEvent(const char *kind, const char *text)
{
    const std::string encoded = encode(text);
    std::fprintf(stdout, "%s %s\n", kind, encoded.c_str());
    std::fflush(stdout);
}

// Bounded because an unbounded read from a pipe this process does not control
// is a memory bug waiting for a hostile parent. No real response, encoded,
// approaches this.
constexpr std::string::size_type maximumLineBytes = 16384;

// One line from the parent, or nothing. Returns false at end of input, which
// is how the parent cancels: it closes the pipe and this program stops asking.
bool readLine(std::string *line)
{
    line->clear();
    int byte = 0;
    while ((byte = std::fgetc(stdin)) != EOF) {
        if (byte == '\n')
            return true;
        if (line->size() >= maximumLineBytes) {
            wipe(*line);
            return false;
        }
        line->push_back(static_cast<char>(byte));
    }
    return false;
}

void onRequest(PolkitAgentSession *, const gchar *request, gboolean echoOn,
               gpointer data)
{
    auto *conversation = static_cast<Conversation *>(data);
    emitEvent(echoOn ? "visible" : "secret", request);

    std::string line;
    if (!readLine(&line)) {
        // The parent went away or refused to answer. Cancelling is the only
        // honest move: an empty response handed to PAM is an attempt the
        // person never made.
        polkit_agent_session_cancel(conversation->session);
        return;
    }

    std::string response;
    if (!decode(line, &response)) {
        wipe(line);
        polkit_agent_session_cancel(conversation->session);
        return;
    }
    wipe(line);

    // Into the session and out of this process, in that order. It is written
    // to no other stream and kept in no other buffer.
    polkit_agent_session_response(conversation->session, response.c_str());
    wipe(response);
}

void onShowInfo(PolkitAgentSession *, const gchar *text, gpointer)
{
    emitEvent("info", text);
}

void onShowError(PolkitAgentSession *, const gchar *text, gpointer)
{
    emitEvent("problem", text);
}

void onCompleted(PolkitAgentSession *, gboolean gainedAuthorization,
                 gpointer data)
{
    auto *conversation = static_cast<Conversation *>(data);
    conversation->decided = true;
    conversation->outcome = gainedAuthorization ? Authenticated : Refused;
    g_main_loop_quit(conversation->loop);
}

} // namespace

int main(int argc, char **argv)
{
    std::string user;
    for (int index = 1; index < argc; ++index) {
        const std::string argument = argv[index];
        if (argument == "--user" && index + 1 < argc) {
            user = argv[++index];
        } else {
            std::fprintf(stderr, "celestina-polkit-converse: unknown argument\n");
            return Unavailable;
        }
    }
    if (user.empty()) {
        std::fprintf(stderr, "celestina-polkit-converse: --user is required\n");
        return Unavailable;
    }

    // The cookie comes down the pipe rather than on argv for the same reason
    // the passphrase does: every process on this machine can read another's
    // command line out of /proc, and a cookie is what polkitd matches an
    // answer against.
    std::string cookie;
    if (!readLine(&cookie) || cookie.empty()) {
        std::fprintf(stderr, "celestina-polkit-converse: no cookie\n");
        return Unavailable;
    }

    PolkitIdentity *identity = polkit_unix_user_new_for_name(user.c_str(),
                                                             nullptr);
    if (!identity) {
        std::fprintf(stderr,
                     "celestina-polkit-converse: no such identity\n");
        return Unavailable;
    }

    Conversation conversation;
    conversation.loop = g_main_loop_new(nullptr, FALSE);
    conversation.session = polkit_agent_session_new(identity, cookie.c_str());
    // The cookie has been handed over; this copy is of no further use to
    // anyone but somebody reading this process's memory.
    wipe(cookie);
    if (!conversation.session) {
        std::fprintf(stderr,
                     "celestina-polkit-converse: no session\n");
        g_object_unref(identity);
        return Unavailable;
    }

    g_signal_connect(conversation.session, "request",
                     G_CALLBACK(onRequest), &conversation);
    g_signal_connect(conversation.session, "show-info",
                     G_CALLBACK(onShowInfo), &conversation);
    g_signal_connect(conversation.session, "show-error",
                     G_CALLBACK(onShowError), &conversation);
    g_signal_connect(conversation.session, "completed",
                     G_CALLBACK(onCompleted), &conversation);

    polkit_agent_session_initiate(conversation.session);
    g_main_loop_run(conversation.loop);

    g_object_unref(conversation.session);
    g_object_unref(identity);
    g_main_loop_unref(conversation.loop);

    // A session that stopped without completing decided nothing, and an
    // undecided attempt is not an authorization.
    if (!conversation.decided)
        return Unavailable;
    return conversation.outcome;
}
