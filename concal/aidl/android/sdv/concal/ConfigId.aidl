package android.sdv.concal;

/**
 * Unique config identifier in the form
 * <full package name like com.company.domain.apex>.<service name like
 * SomeService>/(<instance_name>?)/<config_name>.
 * It does not include VM name, but it can be introduced if
 * required. If a field is not defined, this field matches any characters.
 * Undefined fields allow to match multiple configs.
 *
 * This is a PoC version for the API for the discussions.
 * It is unstable until API council approval.
 */

parcelable ConfigId {
    String companyDomain;
    @nullable String domain;
    @nullable String apex;
    @nullable String service;
    @nullable String instance;
    @nullable String config;
}
