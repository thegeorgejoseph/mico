package server

import (
	"net/http"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/agent"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/doctor"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/logs"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/navigation"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/notifications"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/repos"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/sessions"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/state"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/worktrees"
)

type App struct {
	store         *state.Store
	repos         *repos.Service
	worktrees     *worktrees.Service
	sessions      *sessions.Service
	agents        *agent.Service
	doctor        *doctor.Service
	logs          *logs.Service
	notifications *notifications.Service
	navigation    *navigation.Service
}

func NewApp(store *state.Store, repos *repos.Service, worktrees *worktrees.Service, sessions *sessions.Service, agents *agent.Service, doctor *doctor.Service, logs *logs.Service, notifications *notifications.Service, navigation *navigation.Service) *App {
	return &App{
		store:         store,
		repos:         repos,
		worktrees:     worktrees,
		sessions:      sessions,
		agents:        agents,
		doctor:        doctor,
		logs:          logs,
		notifications: notifications,
		navigation:    navigation,
	}
}

func (a *App) Handler() http.Handler {
	mux := http.NewServeMux()
	a.registerCoreRoutes(mux)
	a.registerRepoRoutes(mux)
	a.registerWorktreeRoutes(mux)
	a.registerSessionRoutes(mux)
	a.registerNotificationRoutes(mux)
	a.registerNavigationRoutes(mux)
	a.registerAgentRoutes(mux)
	return withCORS(a.withRequestLogging(a.withRecover(mux)))
}
