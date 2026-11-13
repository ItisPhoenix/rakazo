package com.rakazo.devlink

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The development link may only be used by the Charlie Wrist Lab build.
 *
 * The guard is the one place that decides this, so it is worth a test of its
 * own: a production build must never be able to talk to a simulator.
 */
class DevLinkLabTest {
  @Test
  fun `the lab package is allowed`() {
    assertTrue(DevLinkLab.isLabPackage("com.rakazo.charliewristlab"))
  }

  @Test
  fun `another package is refused`() {
    assertFalse(DevLinkLab.isLabPackage("com.rakazo.app"))
    assertFalse(DevLinkLab.isLabPackage("com.rakazo.charliewrist"))
    assertFalse(DevLinkLab.isLabPackage("com.rakazo.charliewristlab.debug"))
    assertFalse(DevLinkLab.isLabPackage(""))
    assertFalse(DevLinkLab.isLabPackage(null))
  }

  @Test
  fun `the package name is compared exactly`() {
    assertFalse(DevLinkLab.isLabPackage("COM.RAKAZO.CHARLIEWRISTLAB"))
    assertFalse(DevLinkLab.isLabPackage(" com.rakazo.charliewristlab"))
  }
}
