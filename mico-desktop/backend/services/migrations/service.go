package migrations

import (
	"time"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/services/state"
	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

type Migration interface {
	ID() string
	Run(current *domain.State) error
}

type Runner struct {
	store      *state.Store
	now        func() time.Time
	migrations []Migration
}

func NewRunner(store *state.Store, migrations ...Migration) *Runner {
	return &Runner{
		store:      store,
		now:        time.Now,
		migrations: append([]Migration(nil), migrations...),
	}
}

func (r *Runner) Apply() error {
	for _, migration := range r.migrations {
		if _, err := r.store.Update(func(current *domain.State) error {
			if hasAppliedMigration(current.Migrations, migration.ID()) {
				return nil
			}
			if err := migration.Run(current); err != nil {
				return err
			}
			current.Migrations = append(current.Migrations, domain.AppliedMigration{
				ID:        migration.ID(),
				AppliedAt: r.now().UTC(),
			})
			return nil
		}); err != nil {
			return err
		}
	}
	return nil
}

func hasAppliedMigration(applied []domain.AppliedMigration, id string) bool {
	for _, migration := range applied {
		if migration.ID == id {
			return true
		}
	}
	return false
}
