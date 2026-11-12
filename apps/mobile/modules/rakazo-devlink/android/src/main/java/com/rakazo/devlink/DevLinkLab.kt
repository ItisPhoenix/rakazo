package com.rakazo.devlink

/**
 * The build scope of the Charlie WatchOS development link.
 *
 * The link is a development tool, so it only works in the Charlie Wrist Lab
 * build. Every check reads the package name from here, so the native guard and
 * the JavaScript side can never disagree about which build may talk to a
 * development simulator.
 */
object DevLinkLab {
  /** The package name of the Charlie Wrist Lab build. */
  const val PACKAGE = "com.rakazo.charliewristlab"

  /** True when the given package name belongs to the Charlie Wrist Lab build. */
  fun isLabPackage(packageName: String?): Boolean = packageName == PACKAGE
}
