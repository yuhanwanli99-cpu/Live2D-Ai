import logging
import os
import shutil
import tempfile

from packaging import version
from ruamel.yaml import YAML

from src.open_llm_vtuber.config_manager.utils import load_text_file_with_guess_encoding
from upgrade_codes.upgrade_core.comment_diff_fn import comment_diff_fn
from upgrade_codes.upgrade_core.comment_sync import CommentSynchronizer
from upgrade_codes.upgrade_core.constants import (
    BACKUP_CONF,
    EN_DEFAULT_CONF,
    TEXTS,
    TEXTS_COMPARE,
    TEXTS_MERGE,
    USER_CONF,
    ZH_DEFAULT_CONF,
)
from upgrade_codes.upgrade_core.upgrade_utils import UpgradeUtility
from upgrade_codes.version_manager import VersionUpgradeManager


class ConfigSynchronizer:
    def __init__(self, lang="en", logger=None):
        self.lang = lang
        self.texts = TEXTS[lang]
        self.default_path = ZH_DEFAULT_CONF if lang == "zh" else EN_DEFAULT_CONF
        self.yaml = YAML()
        self.yaml.preserve_quotes = True
        self.user_path = USER_CONF
        self.backup_path = BACKUP_CONF
        self.texts_merge = TEXTS_MERGE.get(lang, TEXTS_MERGE["en"])
        self.texts_compare = TEXTS_COMPARE.get(lang, TEXTS_COMPARE["en"])
        self.logger = logger or logging.getLogger(__name__)
        self.upgrade_utils = UpgradeUtility(self.logger, self.lang)

    def sync_user_config(self) -> None:
        """
        Ensure the user configuration file exists and create a backup if necessary.
        If the user config file does not exist, copy the default config.
        """
        # Check if the user config file exists
        if not os.path.exists(self.user_path):
            self.logger.warning(self.texts["no_config"])
            self.logger.warning(self.texts["copy_default_config"])
            # Copy default config to user path
            shutil.copy2(self.default_path, self.user_path)
            return

        # Create a backup of the user config file
        self.backup_user_config()

    def update_user_config(self) -> None:
        """Update configuration transactionally without deleting user keys.

        The default template is a source of missing defaults, not an allow-list.
        Fork, plugin and future-version keys are therefore preserved. Any failure
        during merge, comment synchronization or version migration restores the
        pre-update backup and is re-raised so callers cannot report false success.
        """
        self.backup_user_config()
        try:
            # Step 1: add missing defaults; unknown user keys are intentionally kept.
            if not self.compare_field_keys():
                self.merge_and_update_user_config()
            else:
                self.logger.info(self.texts["configs_up_to_date"])

            # Step 2: synchronize comments without changing user values/extra keys.
            if not self.compare_comments():
                comment_sync = CommentSynchronizer(
                    self.default_path,
                    self.user_path,
                    self.logger,
                    self.yaml,
                    self.texts_compare,
                )
                comment_sync.sync()
            else:
                self.logger.info(self.texts_compare["comments_up_to_date"])

            # Step 3: run only explicit, versioned migrations.
            new_version = self.get_latest_version()
            old_version = self.get_old_version()
            if old_version != new_version:
                version_upgrade_manager = VersionUpgradeManager(self.lang, self.logger)
                # The manager historically uses the module constant; bind the
                # configured path so tests and alternate deployments stay isolated.
                version_upgrade_manager.user_config = self.user_path
                final_version = version_upgrade_manager.upgrade(old_version)
                self.logger.info(
                    self.texts["version_upgrade_success"].format(
                        old=old_version, new=final_version
                    )
                )
            else:
                self.logger.info(
                    self.texts["version_upgrade_none"].format(version=old_version)
                )
        except Exception:
            self.restore_user_config()
            raise

    def backup_user_config(self):
        backup_path = os.path.abspath(self.backup_path)
        self.logger.info(
            self.texts["backup_user_config"].format(
                user_conf=self.user_path, backup_conf=self.backup_path
            )
        )
        self.logger.debug(self.texts["config_backup_path"].format(path=backup_path))
        shutil.copy2(self.user_path, self.backup_path)

    def restore_user_config(self) -> None:
        """Atomically restore the pre-update configuration backup."""
        if not os.path.exists(self.backup_path):
            raise FileNotFoundError(f"Config backup not found: {self.backup_path}")
        self._atomic_copy(self.backup_path, self.user_path)
        self.logger.warning("Configuration update failed; restored backup.")

    @staticmethod
    def _atomic_copy(source_path: str, destination_path: str) -> None:
        destination_dir = os.path.dirname(os.path.abspath(destination_path))
        os.makedirs(destination_dir, exist_ok=True)
        fd, temp_path = tempfile.mkstemp(prefix=".conf-restore-", dir=destination_dir)
        os.close(fd)
        try:
            shutil.copy2(source_path, temp_path)
            os.replace(temp_path, destination_path)
        finally:
            if os.path.exists(temp_path):
                os.unlink(temp_path)

    def _atomic_dump(self, data) -> None:
        """Dump YAML beside the destination and atomically replace it."""
        destination_dir = os.path.dirname(os.path.abspath(self.user_path))
        os.makedirs(destination_dir, exist_ok=True)
        fd, temp_path = tempfile.mkstemp(
            prefix=".conf-update-", suffix=".yaml", dir=destination_dir
        )
        try:
            with os.fdopen(fd, "w", encoding="utf-8") as stream:
                self.yaml.dump(data, stream)
                stream.flush()
                os.fsync(stream.fileno())
            os.replace(temp_path, self.user_path)
        finally:
            if os.path.exists(temp_path):
                os.unlink(temp_path)

    def merge_and_update_user_config(self):
        try:
            new_keys = self.merge_configs()
            if new_keys:
                self.logger.info(self.texts["merged_config_success"])
                for key in new_keys:
                    self.logger.info(f"  - {key}")
            else:
                self.logger.info(self.texts["merged_config_none"])
        except Exception as e:
            self.logger.error(self.texts["merge_failed"].format(error=e))
            raise

    def merge_configs(self):
        user_config = self.yaml.load(load_text_file_with_guess_encoding(self.user_path))
        default_config = self.yaml.load(
            load_text_file_with_guess_encoding(self.default_path)
        )

        new_keys = []

        def merge(d_user, d_default, path=""):
            for k, v in d_default.items():
                current_path = f"{path}.{k}" if path else k
                if k not in d_user:
                    d_user[k] = v
                    new_keys.append(current_path)
                elif isinstance(v, dict) and isinstance(d_user.get(k), dict):
                    merge(d_user[k], v, current_path)
            return d_user

        merged = merge(user_config, default_config)

        self._atomic_dump(merged)

        for key in new_keys:
            self.logger.info(self.texts_merge["new_config_item"].format(key=key))
        return new_keys

    def collect_all_subkeys(self, d, base_path):
        """Collect all keys in the dictionary d, recursively, with base_path as the prefix."""
        keys = []
        # Only process if d is a dictionary
        if isinstance(d, dict):
            for key, value in d.items():
                current_path = f"{base_path}.{key}" if base_path else key
                keys.append(current_path)
                if isinstance(value, dict):
                    keys.extend(self.collect_all_subkeys(value, current_path))
        return keys

    def get_missing_keys(self, user, default, path=""):
        """Recursively find keys in default that are missing in user."""
        missing = []
        for key, default_val in default.items():
            current_path = f"{path}.{key}" if path else key
            if key not in user:
                missing.append(current_path)
            else:
                user_val = user[key]
                if isinstance(default_val, dict):
                    if isinstance(user_val, dict):
                        missing.extend(
                            self.get_missing_keys(user_val, default_val, current_path)
                        )
                    else:
                        subtree_missing = self.collect_all_subkeys(
                            default_val, current_path
                        )
                        missing.extend(subtree_missing)
        return missing

    def get_extra_keys(self, user, default, path=""):
        """Recursively find keys in user that are not present in default."""
        extra = []
        for key, user_val in user.items():
            current_path = f"{path}.{key}" if path else key
            if key not in default:
                # Only collect subkeys if the value is a dictionary
                if isinstance(user_val, dict):
                    subtree_extra = self.collect_all_subkeys(user_val, current_path)
                    extra.extend(subtree_extra)
                extra.append(current_path)
            else:
                default_val = default[key]
                if isinstance(user_val, dict) and isinstance(default_val, dict):
                    extra.extend(
                        self.get_extra_keys(user_val, default_val, current_path)
                    )
                elif isinstance(user_val, dict):
                    subtree_extra = self.collect_all_subkeys(user_val, current_path)
                    extra.extend(subtree_extra)
        return extra

    def delete_extra_keys(self):
        """Refuse destructive template-based cleanup.

        Unknown keys can belong to this fork, a plugin or a newer application
        version. They may only be removed by an explicit version migration.
        """
        raise RuntimeError(
            "Refusing to delete user configuration keys absent from the default template"
        )

    def compare_field_keys(self) -> bool:
        """Compare field structure differences (missing/extra keys)"""

        def field_compare_fn(user, default):
            missing = self.get_missing_keys(user, default)
            extra = self.get_extra_keys(user, default)

            if missing:
                self.logger.warning(
                    self.texts_compare["missing_keys"].format(keys=", ".join(missing))
                )
            if extra:
                self.logger.warning(
                    self.texts_compare["extra_keys"].format(keys=", ".join(extra))
                )
                self.logger.info(
                    "Preserving user/fork configuration keys absent from the default template."
                )
            # Extra keys are valid user extensions and do not make the config stale.
            return (not missing, missing)

        return self.upgrade_utils.compare_dicts(
            name="keys",
            get_a=lambda: self.yaml.load(
                load_text_file_with_guess_encoding(self.user_path)
            ),
            get_b=lambda: self.yaml.load(
                load_text_file_with_guess_encoding(self.default_path)
            ),
            compare_fn=field_compare_fn,
        )

    def compare_comments(self) -> bool:
        return self.upgrade_utils.compare_dicts(
            name="comments",
            get_a=lambda: load_text_file_with_guess_encoding(self.user_path),
            get_b=lambda: load_text_file_with_guess_encoding(self.default_path),
            compare_fn=comment_diff_fn,
        )

    def get_latest_version(self):
        with open(self.default_path, "r", encoding="utf-8") as f:
            default_config = self.yaml.load(f)
        return default_config.get("system_config", {}).get("conf_version", "")

    def get_old_version(self) -> str:
        """
        Extract the old version from backup config.
        If missing or too old (< v1.1.1), fallback to v1.1.1.
        """
        fallback_version = "v1.1.1"
        try:
            yaml = YAML()
            with open(self.backup_path, "r", encoding="utf-8") as f:
                backup_conf = yaml.load(f)
                raw_version = backup_conf.get("system_config", {}).get(
                    "conf_version", fallback_version
                )

                if version.parse(raw_version) < version.parse(fallback_version):
                    self.logger.warning(
                        self.texts["version_too_old"].format(
                            found=raw_version, adjusted=fallback_version
                        )
                    )
                    return fallback_version

                self.logger.info(
                    self.texts["backup_used_version"].format(backup_version=raw_version)
                )
                return raw_version
        except Exception as e:  # noqa: BLE001
            self.logger.warning(
                self.texts["backup_read_error"].format(
                    version=fallback_version, error=e
                )
            )
            return fallback_version
