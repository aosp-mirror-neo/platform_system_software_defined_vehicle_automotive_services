package android.sdv.config;

/**
 * Unique config identifier in the form
 * "<com.company.domain.apex_name>(.<instance_id>)?.<config_name>"
 * TODO: clarify if the instance could be removed from the config id.
 *
 * This is a PoC version for the API for the discussions.
 * It is unstable until API council approval.
 */
parcelable ConfigId {
    String companyDomain;
    @nullable String domain;
    @nullable String apex;
    @nullable String instance;
    @nullable String config;
}
