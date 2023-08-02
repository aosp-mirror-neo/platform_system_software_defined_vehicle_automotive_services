package android.sdv.diagnostics.file_transfer;

/**
 * Handler for a file transfer related services.
 *
 * Handles file transfer operations on top of the following services:
 * - RequestDownload (UDS 0x34)
 * - TransferData (UDS 0x36)
 * - RequestTransferExit (UDS 0x37)
 * - RequestFileTransfer (UDS 0x38)
 */
interface IFileTransferHandler {
    /**
     * Add new file.
     *
     * @param[in] filePath id of the file to add.
     *
     * @return absolute path to the file to add.
     */
    String addFile(in String filePath, in long fileSize);

    /**
     * Delete file.
     *
     * @param[in] filePath id of the file to delete.
     *
     * @return absolute path to the file to delete.
     */
    void deleteFile(in String filePath);

    /**
     * Replace file.
     *
     * @param[in] filePath id of the file to replace.
     * @param[in] fileSize size of the replacing file.
     *
     * @return absolute path to the file to replace.
     */
    String replaceFile(in String filePath, in long fileSize);

    /**
     * Response for the `readFile` callback.
     */
    parcelable ReadFileResponse {
        /**
         * Absolute path to the file to read.
         */
        String filePath;

        /**
         * Size of the file being read.
         */
        long fileSize;
    }

    /**
     * Read file.
     *
     * @param[in] filePath id of the file to read.
     *
     * @return ReadFileResponse with data about the file to read.
     */
    ReadFileResponse readFile(in String filePath);

    /**
     * Response for the `resumeFile` callback.
     */
    parcelable ResumeFileResponse {
        /**
         * Absolute path to the file to resume.
         */
        String filePath;

        /**
         * Size of the file to resume.
         */
        long fileSize;

        /**
         * Position at which to resume the reading/writing of the file.
         */
        long filePosition;
    }

    /**
     * Resume reading/writing the file.
     *
     * @param[in] filePath id of the file to resume.
     *
     * @return ResumeFileResponse with data about the file to resume.
     */
    ResumeFileResponse resumeFile(in String filePath);

    /**
     * Finish file transfer operation.
     *
     * Called to indicate the finish of the file read/write operation initiated by one of
     * the other callbacks (`addFile`/`replaceFile`/`readFile`/`resumeFile`).
     *
     * This method corresponds to RequestTransferExit (UDS 0x37) service.
     *
     * @param[in] filePath id of the file to finish operation on, this should be the
     *            path used to initiate the operation.
     */
    void requestTransferExit(in String filePath);
}
