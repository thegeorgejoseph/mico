package main

import (
	"errors"
	"flag"
	"fmt"
	"log"
	"net/http"
	"os"
	"path/filepath"
	"time"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/agent"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/doctor"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/logs"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/migrations"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/navigation"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/notifications"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/repos"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/runtimepath"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/sessions"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/state"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/worktrees"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/server"
)

func main() {
	defaultRoot := defaultDataRoot()
	addr := flag.String("addr", "127.0.0.1:48011", "HTTP address for the desktop backend")
	statePath := flag.String("state", filepath.Join(defaultRoot, "state.json"), "path to state JSON")
	worktreesRoot := flag.String("worktrees", filepath.Join(defaultRoot, "worktrees"), "root directory for worktrees")
	legacyStatePath := flag.String("legacy-state", migrations.DefaultLegacyStatePath(), "path to existing Rust CLI state JSON to import")
	flag.Parse()

	if err := runtimepath.NewService().Configure(); err != nil {
		log.Printf("warning: failed to configure runtime PATH: %v", err)
	}

	store := state.NewStore(*statePath)
	logService := logs.NewService(store)
	if err := migrations.NewRunner(store, migrations.NewLegacyStateMigration(*legacyStatePath)).Apply(); err != nil {
		log.Printf("warning: failed to apply desktop migrations: %v", err)
		logService.Record(domain.LogWarn, "migrations", "apply", err.Error(), map[string]string{"path": *legacyStatePath})
	} else {
		logService.Record(domain.LogInfo, "migrations", "apply", "Desktop migrations checked.", map[string]string{"path": *legacyStatePath})
	}
	notifier := notifications.NewService(store)
	runner := repos.ExecRunner{}
	repoService := repos.NewService(store, runner, notifier)
	worktreeService := worktrees.NewService(store, repoService, runner, notifier, *worktreesRoot)
	sessionService := sessions.NewService(store, worktreeService, sessions.ExecStarter{}, notifier)
	agentService := agent.NewServiceWithExecutor(store, agent.ExecCommandExecutor{})
	navigationService := navigation.NewService(store)
	doctorService := doctor.NewService()
	app := server.NewApp(store, repoService, worktreeService, sessionService, agentService, doctorService, logService, notifier, navigationService)
	httpServer := &http.Server{
		Addr:              *addr,
		Handler:           app.Handler(),
		ReadHeaderTimeout: 5 * time.Second,
		IdleTimeout:       60 * time.Second,
	}

	logService.Record(domain.LogInfo, "server", "listen", "mico desktop backend listening.", map[string]string{"addr": *addr})
	if err := httpServer.ListenAndServe(); err != nil && !errors.Is(err, http.ErrServerClosed) {
		log.Printf("mico desktop backend failed: %v", err)
		logService.Record(domain.LogError, "server", "listen", err.Error(), map[string]string{"addr": *addr})
		os.Exit(1)
	}
}

func defaultDataRoot() string {
	home, err := os.UserHomeDir()
	if err != nil {
		return filepath.Join(os.TempDir(), "mico-desktop")
	}
	return fmt.Sprintf("%s/.mico-desktop", home)
}
