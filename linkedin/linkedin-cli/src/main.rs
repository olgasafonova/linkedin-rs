use clap::{CommandFactory, Parser};

mod auth;
mod cli;
mod company;
mod connections;
mod events;
mod feed;
mod graphql_print;
mod messages;
mod notifications;
mod profile;
mod search;
mod session;
mod spam;
mod util;

use cli::{
    AuthAction, Cli, Commands, CompanyAction, ConnectionsAction, EventsAction, FeedAction,
    MessagesAction, NotificationsAction, ProfileAction, SearchAction,
};
use util::exit_on_err;

use auth::{cmd_auth_login, cmd_auth_logout, cmd_auth_status};
use company::{cmd_company_followers, cmd_company_view};
use connections::{cmd_connections_accept, cmd_connections_invitations, cmd_connections_invite, cmd_connections_invite_batch, cmd_connections_list, cmd_connections_withdraw};
use events::{cmd_event_attendees, cmd_event_view};
use feed::{cmd_feed_comment, cmd_feed_comments, cmd_feed_list, cmd_feed_my_posts, cmd_feed_post, cmd_feed_react, cmd_feed_reactions, cmd_feed_read, cmd_feed_stats, cmd_feed_unreact, cmd_feed_view};
use messages::{cmd_inbox, cmd_messages_list, cmd_messages_read, cmd_messages_reply, cmd_messages_send, cmd_who};
use notifications::{cmd_notifications_list, cmd_notifications_mentions};
use profile::{cmd_profile_audit, cmd_profile_me, cmd_profile_view, cmd_profile_viewers, cmd_profile_visit};
use search::{cmd_search_invite, cmd_search_jobs, cmd_search_people, cmd_search_posts, cmd_search_react, cmd_search_view};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Auth { action } => match action {
            AuthAction::Login { li_at } => exit_on_err(cmd_auth_login(li_at).await),
            AuthAction::Status { local } => exit_on_err(cmd_auth_status(local).await),
            AuthAction::Logout => exit_on_err(cmd_auth_logout()),
        },
        Commands::Profile { action } => match action {
            ProfileAction::Me { json } => exit_on_err(cmd_profile_me(json).await),
            ProfileAction::View {
                public_id,
                json,
                summary,
            } => exit_on_err(cmd_profile_view(&public_id, json, summary).await),
            ProfileAction::Visit { public_id, json } => {
                exit_on_err(cmd_profile_visit(&public_id, json).await)
            }
            ProfileAction::Viewers { json } => exit_on_err(cmd_profile_viewers(json).await),
            ProfileAction::Audit { json } => exit_on_err(cmd_profile_audit(json).await),
        },
        Commands::Messages { action } => match action {
            MessagesAction::List {
                count,
                before,
                json,
            } => exit_on_err(cmd_messages_list(count, before, json).await),
            MessagesAction::Read {
                conversation_id,
                before,
                json,
            } => exit_on_err(cmd_messages_read(&conversation_id, before, json).await),
            MessagesAction::Send {
                recipient,
                message,
                yes,
                json,
            } => exit_on_err(cmd_messages_send(&recipient, &message, yes, json).await),
            MessagesAction::Reply {
                conversation_id,
                message,
                yes,
                json,
            } => exit_on_err(cmd_messages_reply(&conversation_id, &message, yes, json).await),
        },
        Commands::Feed { action } => match action {
            FeedAction::List {
                count,
                start,
                author,
                keyword,
                json,
            } => exit_on_err(
                cmd_feed_list(start, count, author.as_deref(), keyword.as_deref(), json).await,
            ),
            FeedAction::Comments { index, count, json } => {
                exit_on_err(cmd_feed_comments(index, count, json).await)
            }
            FeedAction::Read { index, json } => exit_on_err(cmd_feed_read(index, json)),
            FeedAction::View { activity_urn, json } => {
                exit_on_err(cmd_feed_view(&activity_urn, json).await)
            }
            FeedAction::React {
                post_urn,
                reaction_type,
                yes,
                json,
            } => exit_on_err(cmd_feed_react(&post_urn, &reaction_type, yes, json).await),
            FeedAction::Unreact {
                post_urn,
                reaction_type,
                json,
            } => exit_on_err(cmd_feed_unreact(&post_urn, &reaction_type, json).await),
            FeedAction::Comment {
                post_urn,
                text,
                yes,
                json,
            } => exit_on_err(cmd_feed_comment(&post_urn, &text, yes, json).await),
            FeedAction::MyPosts { count, start, json } => {
                exit_on_err(cmd_feed_my_posts(start, count, json).await)
            }
            FeedAction::Reactions {
                post_urn,
                from_list,
                count,
                start,
                json,
            } => exit_on_err(
                cmd_feed_reactions(post_urn.as_deref(), from_list, start, count, json).await,
            ),
            FeedAction::Stats { json } => exit_on_err(cmd_feed_stats(json).await),
            FeedAction::Post {
                text,
                visibility,
                yes,
                json,
            } => exit_on_err(cmd_feed_post(&text, &visibility, yes, json).await),
        },
        Commands::Connections { action } => match action {
            ConnectionsAction::List {
                count,
                start,
                all,
                keyword,
                json,
            } => {
                exit_on_err(cmd_connections_list(start, count, all, keyword.as_deref(), json).await)
            }
            ConnectionsAction::Invite {
                public_id_or_urn,
                message,
                json,
            } => exit_on_err(
                cmd_connections_invite(&public_id_or_urn, message.as_deref(), json).await,
            ),
            ConnectionsAction::InviteBatch {
                from_file,
                message,
                pacing_ms,
                stop_on_error,
            } => exit_on_err(
                cmd_connections_invite_batch(
                    &from_file,
                    message.as_deref(),
                    pacing_ms,
                    stop_on_error,
                )
                .await,
            ),
            ConnectionsAction::Invitations { count, start, json } => {
                exit_on_err(cmd_connections_invitations(start, count, json).await)
            }
            ConnectionsAction::Accept {
                invitation_id,
                secret,
                json,
            } => exit_on_err(cmd_connections_accept(&invitation_id, &secret, json).await),
            ConnectionsAction::Withdraw {
                invitation_id,
                secret,
                json,
            } => exit_on_err(cmd_connections_withdraw(&invitation_id, &secret, json).await),
        },
        Commands::Search { action } => match action {
            SearchAction::People {
                keywords,
                count,
                start,
                json,
            } => exit_on_err(cmd_search_people(&keywords, start, count, json).await),
            SearchAction::Jobs {
                keywords,
                count,
                start,
                json,
            } => exit_on_err(cmd_search_jobs(&keywords, start, count, json).await),
            SearchAction::Posts {
                keywords,
                count,
                start,
                json,
            } => exit_on_err(cmd_search_posts(&keywords, start, count, json).await),
            SearchAction::React {
                index,
                reaction_type,
                json,
            } => exit_on_err(cmd_search_react(index, &reaction_type, json).await),
            SearchAction::View { index, json } => exit_on_err(cmd_search_view(index, json).await),
            SearchAction::Invite {
                index,
                message,
                json,
            } => exit_on_err(cmd_search_invite(index, message.as_deref(), json).await),
        },
        Commands::Events { action } => match action {
            EventsAction::View { event_id, json } => {
                exit_on_err(cmd_event_view(&event_id, json).await)
            }
            EventsAction::Attendees {
                event_id,
                count,
                start,
                json,
            } => exit_on_err(cmd_event_attendees(&event_id, start, count, json).await),
        },
        Commands::Company { action } => match action {
            CompanyAction::View { slug, json } => exit_on_err(cmd_company_view(&slug, json).await),
            CompanyAction::Followers {
                slug,
                count,
                start,
                json,
            } => exit_on_err(cmd_company_followers(&slug, start, count, json).await),
        },
        Commands::Notifications { action } => match action {
            NotificationsAction::List { count, start, json } => {
                exit_on_err(cmd_notifications_list(start, count, json).await)
            }
            NotificationsAction::Mentions { index, json } => {
                exit_on_err(cmd_notifications_mentions(index, json).await)
            }
        },
        Commands::Inbox { json, all } => exit_on_err(cmd_inbox(json, all).await),
        Commands::Who { company, json } => exit_on_err(cmd_who(&company, json).await),
        Commands::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "li", &mut std::io::stdout());
        }
    }
}
